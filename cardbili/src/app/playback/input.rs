use slint::ComponentHandle as _;
use super::super::*;

impl App {
    pub(in crate::app) fn play_key(&mut self, k: &str, repeat: bool) {
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
                    self.ui.global::<crate::Player>().set_progress(if p.duration > 0.0 {
                        (position / p.duration) as f32
                    } else {
                        0.0
                    });
                    self.ui.global::<crate::Player>().set_time_text(
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
