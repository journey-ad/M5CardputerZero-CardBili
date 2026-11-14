use slint::ComponentHandle as _;
use super::super::*;

impl App {
    pub(in crate::app) fn tick(&mut self) {
        self.tick_input();
        self.tick_like();
        self.report_history(false, false);
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
                        self.danmaku_view
                            .update(engine, pp.position(self.frames_in_seg));
                    }
                    self.status = "加载中…".into();
                    self.ui.global::<crate::Player>().set_has_frame(false);
                }
            }
        }
        if self.page == Page::Home && self.tab.is_video() {
            if let Some(t) = self.esc_press {
                if t.elapsed() >= Duration::from_millis(2000) {
                    self.quit = true;
                }
            }
        }
        let g_status = self.ui.global::<crate::Status>();
        if let Some(t) = self.bar_until {
            if Instant::now() >= t {
                g_status.set_bar("".into());
                g_status.set_bar_sticky(false);
                self.bar_until = None;
                self.back_hint = false;
            }
        }
    }
}
