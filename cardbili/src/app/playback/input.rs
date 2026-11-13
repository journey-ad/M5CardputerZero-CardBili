use super::super::*;

impl App {
    pub(in crate::app) fn play_key(&mut self, k: &str) {
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
                    p.pause_toggle();
                }
                "LEFT" | "z" => {
                    self.left_hold = Some(Instant::now());
                    self.left_rep = Instant::now();
                    let base = self
                        .seek_pending
                        .unwrap_or_else(|| p.position(self.frames_in_seg));
                    let t = (base - 10.0).max(0.0);
                    self.ui.set_progress(if p.duration > 0.0 {
                        (t / p.duration) as f32
                    } else {
                        0.0
                    });
                    self.ui.set_time_text(
                        format!("{} / {}", model::mmss(t), model::mmss(p.duration))
                            .as_str()
                            .into(),
                    );
                    self.seek_pending = Some(t);
                    self.seek_deadline = Some(Instant::now() + Duration::from_millis(300));
                }
                "RIGHT" | "c" => {
                    self.right_hold = Some(Instant::now());
                    self.right_rep = Instant::now();
                    let base = self
                        .seek_pending
                        .unwrap_or_else(|| p.position(self.frames_in_seg));
                    let t = (base + 10.0).min(p.duration.max(0.0));
                    self.ui.set_progress(if p.duration > 0.0 {
                        (t / p.duration) as f32
                    } else {
                        0.0
                    });
                    self.ui.set_time_text(
                        format!("{} / {}", model::mmss(t), model::mmss(p.duration))
                            .as_str()
                            .into(),
                    );
                    self.seek_pending = Some(t);
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
