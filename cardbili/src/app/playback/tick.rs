use super::super::*;

impl App {
    pub(in crate::app) fn tick(&mut self) {
        self.tick_like();
        // 方向键停止输入 300ms 后提交跳转并显示加载状态
        if let Some(dl) = self.seek_deadline {
            if Instant::now() >= dl {
                self.seek_deadline = None;
                if let (Some(pp), Some(t)) = (self.player.as_mut(), self.seek_pending.take()) {
                    pp.seek(t);
                    self.frames_in_seg = 0;
                    if let Some(engine) = self
                        .danmaku
                        .as_ref()
                        .filter(|_| self.settings.danmaku_enabled)
                    {
                        render_danmaku(
                            &self.ui,
                            engine,
                            pp.position(self.frames_in_seg),
                            &mut self.dm_cache,
                        );
                    }
                    self.ui.set_status("加载中…".into());
                    self.ui.set_has_frame(false);
                }
            }
        }
        // 长按方向键超过 350ms 后，每 120ms 调整 2 秒
        if self.page == 1 {
            if let Some(p) = self.player.as_mut() {
                let mut step = 0.0f64;
                if let Some(start) = self.left_hold {
                    if start.elapsed() >= Duration::from_millis(350)
                        && self.left_rep.elapsed() >= Duration::from_millis(120)
                    {
                        self.left_rep = Instant::now();
                        step = -2.0;
                    }
                }
                if let Some(start) = self.right_hold {
                    if start.elapsed() >= Duration::from_millis(350)
                        && self.right_rep.elapsed() >= Duration::from_millis(120)
                    {
                        self.right_rep = Instant::now();
                        step = 2.0;
                    }
                }
                if step != 0.0 {
                    let base = self
                        .seek_pending
                        .unwrap_or_else(|| p.position(self.frames_in_seg));
                    let t = (base + step).clamp(0.0, p.duration.max(0.0));
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
            }
        }
        if self.page == 0 && self.tab < 3 {
            if let Some(t) = self.esc_press {
                if t.elapsed() >= Duration::from_millis(2000) {
                    self.quit = true;
                }
            }
        }
        if let Some(t) = self.bar_until {
            if Instant::now() >= t {
                self.ui.set_bar("".into());
                self.ui.set_bar_sticky(false);
                self.bar_until = None;
            }
        }
        if self.page == 3 && !self.ui.get_detail_loading() {
            if let Some((step, since)) = self.detail_hold {
                if since.elapsed() >= Duration::from_millis(350)
                    && self.detail_repeat.elapsed() >= Duration::from_millis(120)
                {
                    self.detail_repeat = Instant::now();
                    self.detail_scroll = (self.detail_scroll + step).clamp(0.0, self.detail_maxv);
                    self.ui.set_detail_scroll(self.detail_scroll as f32);
                    sync_thumb(&self.ui, self.detail_scroll, self.detail_maxv);
                }
            }
        } else if self.page != 6 {
            self.detail_hold = None;
        }
    }
}
