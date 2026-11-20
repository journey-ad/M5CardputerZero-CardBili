use super::super::*;
use slint::ComponentHandle as _;

impl App {
    pub(in crate::app) fn tick(&mut self) {
        self.tick_super_chat();
        if let Some(player) = self.player.as_ref().filter(|player| player.stalled()) {
            if self.live.is_some() {
                self.recover_live(crate::i18n::tr("直播画面长时间未更新"));
            } else {
                self.playback_failed(player.gen, crate::i18n::tr("播放超时，按播放键重试").into());
            }
        }
        self.tick_live_recovery();
        if let Some(connection) = self.live_danmaku.as_ref() {
            for event in connection.events.try_iter().take(64) {
                match event {
                    crate::live_danmaku::Event::Danmaku(dm) => {
                        if let Some(player) = self.player.as_ref().filter(|player| {
                            !player.paused
                                && !player.finished
                                && self.frames_in_seg > 0
                                && self.controls.danmaku_enabled
                        }) {
                            self.live_engine
                                .push(dm, player.position(self.frames_in_seg));
                        }
                    }
                    crate::live_danmaku::Event::Audience { text, initial } => {
                        let player = self.ui.global::<crate::Player>();
                        if !initial || player.get_live_audience().is_empty() {
                            player.set_live_audience(text.into());
                        }
                    }
                    crate::live_danmaku::Event::Unavailable => {
                        self.toast(crate::i18n::tr("直播弹幕连接失败，正在重连…"))
                    }
                }
            }
        }
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
                        .filter(|_| self.controls.danmaku_enabled)
                    {
                        self.danmaku_view
                            .update(engine, pp.position(self.frames_in_seg));
                    }
                    self.status = crate::i18n::tr("加载中…").into();
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
