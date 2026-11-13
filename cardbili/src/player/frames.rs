use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
struct State {
    paused: bool,
    stopped: bool,
    #[cfg(not(target_os = "linux"))]
    next: Option<Instant>,
    latest: Option<(u64, Vec<u8>)>,
    audio_sample: Option<(f64, Instant)>,
    position: f64,
}

impl State {
    fn position(&mut self, now: Instant) -> f64 {
        if !self.paused {
            if let Some((time, sampled)) = self.audio_sample {
                // 插值覆盖两次音频采样之间的时间，断流时最多推进 100ms
                let elapsed = now.duration_since(sampled).as_secs_f64().min(0.1);
                self.position = self.position.max(time + elapsed);
            }
        }
        self.position
    }
}

#[derive(Default)]
pub(super) struct Frames {
    state: Mutex<State>,
    changed: Condvar,
}

impl Frames {
    pub fn pause(&self, paused: bool) {
        let mut state = self.state.lock().unwrap();
        state.position(Instant::now());
        state.paused = paused;
        state.audio_sample = None;
        #[cfg(not(target_os = "linux"))]
        {
            state.next = None;
        }
        self.changed.notify_all();
    }

    #[cfg(target_os = "linux")]
    pub fn update_audio_clock(&self, time: f64) {
        if !time.is_finite() || time < 0.0 {
            return;
        }
        let mut state = self.state.lock().unwrap();
        if !state.paused {
            state.audio_sample = Some((time, Instant::now()));
            self.changed.notify_all();
        }
    }

    pub fn position(&self, _index: u64) -> f64 {
        let mut state = self.state.lock().unwrap();
        #[cfg(not(target_os = "linux"))]
        if !state.paused {
            state.position = state
                .position
                .max(_index.saturating_sub(1) as f64 / super::FPS);
        }
        state.position(Instant::now())
    }

    pub fn stop(&self) {
        self.state.lock().unwrap().stopped = true;
        self.changed.notify_all();
    }

    pub fn publish(&self, index: u64, pixels: Vec<u8>) -> Option<bool> {
        let mut state = self.state.lock().unwrap();
        loop {
            if state.stopped {
                return None;
            }
            // 首帧可在加载中暂停时作为预览
            if state.paused && index > 1 {
                state = self.changed.wait(state).unwrap();
                continue;
            }
            #[cfg(target_os = "linux")]
            if index > 1 {
                if state.audio_sample.is_none() {
                    state = self.changed.wait(state).unwrap();
                    continue;
                }
                let ahead = (index - 1) as f64 / super::FPS - state.position(Instant::now());
                if ahead > 0.002 {
                    let wait = Duration::from_secs_f64(ahead.min(0.05));
                    state = self.changed.wait_timeout(state, wait).unwrap().0;
                    continue;
                }
                if ahead < -1.0 / super::FPS {
                    return Some(false);
                }
                break;
            }
            #[cfg(not(target_os = "linux"))]
            if let Some(next) = state.next {
                if let Some(wait) = next.checked_duration_since(Instant::now()) {
                    state = self.changed.wait_timeout(state, wait).unwrap().0;
                    continue;
                }
            }
            break;
        }
        #[cfg(not(target_os = "linux"))]
        {
            let interval = Duration::from_secs_f64(1.0 / super::FPS);
            state.next = Some(state.next.unwrap_or_else(Instant::now) + interval);
        }
        let notify = state.latest.is_none();
        state.latest = Some((index, pixels));
        Some(notify)
    }

    pub fn take(&self) -> Option<(u64, Vec<u8>)> {
        self.state.lock().unwrap().latest.take()
    }
}
