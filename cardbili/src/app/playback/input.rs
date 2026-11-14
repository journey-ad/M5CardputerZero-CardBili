use super::super::*;
use slint::ComponentHandle as _;

impl App {
    // 直播支持暂停、重连、弹幕开关、音量与返回
    fn live_key(&mut self, k: &str) {
        if k == "r" {
            self.requested_live = self.live.as_ref().map(|info| info.room_id);
            return;
        }
        if k == "d" {
            self.toggle_danmaku();
            return;
        }
        if let Some(p) = self.player.as_mut() {
            match k {
                "SPACE" | "ENTER" => {
                    if p.finished || p.paused {
                        self.requested_live = self.live.as_ref().map(|info| info.room_id);
                    } else {
                        p.pause_toggle();
                    }
                }
                "UP" | "f" | "w" => {
                    let _ = self.volume_queue.try_send(10);
                }
                "DOWN" | "x" | "s" => {
                    let _ = self.volume_queue.try_send(-10);
                }
                "ESC" | "BACKSPACE" => self.go_back = true,
                _ => {}
            }
        } else if matches!(k, "SPACE" | "ENTER") {
            self.requested_live = self.live.as_ref().map(|info| info.room_id);
        } else if matches!(k, "ESC" | "BACKSPACE") {
            self.go_back = true;
        }
    }

    pub(in crate::app) fn play_key(&mut self, k: &str, repeat: bool) {
        if self.ui.global::<crate::Player>().get_live() {
            return self.live_key(k);
        }
        if k == "p" {
            self.sections.open = true;
            self.sections.dirty = true;
            return;
        }
        match k {
            "l" => {
                self.like_press();
                return;
            }
            "LIKE_UP" => {
                self.like_release(false);
                return;
            }
            "LIKE_CANCEL" => {
                self.like_release(true);
                return;
            }
            "b" => {
                self.change_speed();
                return;
            }
            "d" => {
                self.toggle_danmaku();
                return;
            }
            "r" => {
                self.requested_comments = self
                    .current_play
                    .as_ref()
                    .map(|info| info.aid)
                    .filter(|aid| *aid > 0);
                return;
            }
            _ => {}
        }
        if matches!(k, "SPACE" | "ENTER" | "LEFT" | "z" | "RIGHT" | "c") {
            self.report_history(true, false);
        }
        let g_player = self.ui.global::<crate::Player>();
        if let Some(p) = self.player.as_mut() {
            match k {
                "SPACE" | "ENTER" => {
                    if p.finished {
                        // 播放结束后再次触发播放，从头开始
                        p.paused = false;
                        self.seek_pending = Some(0.0);
                        self.seek_deadline = Some(Instant::now());
                        crate::diagnostics::write(format_args!("[播放] 重播 gen={}", p.gen));
                    } else {
                        p.pause_toggle();
                    }
                }
                "LEFT" | "RIGHT" => {
                    let step = if repeat { 2.0 } else { 10.0 };
                    let step = if k == "LEFT" { -step } else { step };
                    let base = self
                        .seek_pending
                        .unwrap_or_else(|| p.position(self.frames_in_seg));
                    let position = (base + step).clamp(0.0, p.duration.max(0.0));
                    g_player.set_progress(if p.duration > 0.0 {
                        (position / p.duration) as f32
                    } else {
                        0.0
                    });
                    g_player.set_time_text(
                        format!("{} / {}", model::mmss(position), model::mmss(p.duration)).into(),
                    );
                    self.seek_pending = Some(position);
                    self.seek_deadline = Some(Instant::now() + Duration::from_millis(300));
                }
                "UP" | "f" | "w" => {
                    let _ = self.volume_queue.try_send(10);
                }
                "DOWN" | "x" | "s" => {
                    let _ = self.volume_queue.try_send(-10);
                }
                "u" => {
                    self.requested_uploader = self
                        .current_play
                        .as_ref()
                        .map(|info| info.owner.clone())
                        .filter(|owner| !owner.mid.is_empty());
                }
                "ESC" | "BACKSPACE" => {
                    self.go_back = true;
                }
                _ => {}
            }
        } else if matches!(k, "ESC" | "BACKSPACE") {
            self.go_back = true;
        }
    }
}
