use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
struct State {
    paused: bool,
    stopped: bool,
    next: Option<Instant>,
    latest: Option<(u64, Vec<u8>)>,
}

#[derive(Default)]
pub(super) struct Frames {
    state: Mutex<State>,
    changed: Condvar,
}

impl Frames {
    pub fn pause(&self, paused: bool) {
        let mut state = self.state.lock().unwrap();
        state.paused = paused;
        state.next = None;
        self.changed.notify_all();
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
            if let Some(next) = state.next {
                if let Some(wait) = next.checked_duration_since(Instant::now()) {
                    state = self.changed.wait_timeout(state, wait).unwrap().0;
                    continue;
                }
            }
            break;
        }
        let now = Instant::now();
        let interval = Duration::from_secs_f64(1.0 / super::FPS);
        let next = state.next.unwrap_or(now) + interval;
        state.next = Some(if next > now { next } else { now + interval });
        let notify = state.latest.is_none();
        state.latest = Some((index, pixels));
        Some(notify)
    }

    pub fn take(&self) -> Option<(u64, Vec<u8>)> {
        self.state.lock().unwrap().latest.take()
    }
}
