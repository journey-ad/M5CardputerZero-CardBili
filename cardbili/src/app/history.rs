use super::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

struct Report {
    bvid: String,
    cid: i64,
    progress: i64,
    cookies: String,
}

enum Request {
    Report(Report),
    Flush(mpsc::Sender<()>),
}

pub(super) struct History {
    queue: mpsc::SyncSender<Request>,
    enabled: Arc<AtomicBool>,
    last: Option<(String, i64, Instant)>,
}

impl History {
    pub fn new(enabled: bool) -> Self {
        let enabled = Arc::new(AtomicBool::new(enabled));
        let active = enabled.clone();
        let (queue, requests) = mpsc::sync_channel(8);
        thread::spawn(move || {
            for request in requests {
                match request {
                    Request::Report(report) if active.load(Ordering::Relaxed) => {
                        if let Err(error) = crate::api::playback::heartbeat(
                            &report.bvid,
                            report.cid,
                            report.progress,
                            &report.cookies,
                        ) {
                            eprintln!("观看历史上报失败: {error}");
                        }
                    }
                    Request::Flush(done) => {
                        let _ = done.send(());
                    }
                    _ => {}
                }
            }
        });
        Self {
            queue,
            enabled,
            last: None,
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
        self.last = None;
    }

    pub fn flush(&self) {
        let (done, completed) = mpsc::channel();
        if self.queue.try_send(Request::Flush(done)).is_ok() {
            let _ = completed.recv_timeout(Duration::from_secs(5));
        }
    }
}

impl App {
    pub(super) fn report_history(&mut self, force: bool, completed: bool) {
        if !self.settings.record_history || self.frames_in_seg == 0 {
            return;
        }
        let (Some(info), Some(player), Some(cookies)) =
            (&self.current_play, &self.player, &self.cookies)
        else {
            return;
        };
        let progress = if completed {
            -1
        } else {
            player.position(self.frames_in_seg) as i64
        };
        if progress == 0 || (!completed && player.finished) || (!force && player.paused) {
            return;
        }
        if let Some((bvid, previous, sent)) = &self.history.last {
            if *bvid == info.bvid
                && (*previous == progress || (!force && sent.elapsed() < Duration::from_secs(5)))
            {
                return;
            }
        }
        let report = Report {
            bvid: info.bvid.clone(),
            cid: info.cid,
            progress,
            cookies: cookies.clone(),
        };
        if self.history.queue.try_send(Request::Report(report)).is_ok() {
            self.history.last = Some((info.bvid.clone(), progress, Instant::now()));
            self.loaded[3] = false;
        }
    }
}
