use super::super::*;
use slint::ComponentHandle as _;

impl App {
    pub(in crate::app) fn sync_playback(&mut self) {
        // --- 应用最新帧 ---
        if let Some((frame_gen, pixels)) = self.latest.take() {
            if self.page == Page::Player
                && self
                    .player
                    .as_ref()
                    .is_some_and(|player| player.gen == frame_gen)
            {
                let g_player = self.ui.global::<crate::Player>();
                g_player.set_frame(slint::Image::from_rgb8(pixels));
                g_player.set_has_frame(true);
                if self.live.is_some() && self.settings.danmaku_enabled {
                    if let Some(player) = &self.player {
                        self.danmaku_view.update_rows(
                            self.live_engine
                                .rows_at(player.position(self.frames_in_seg)),
                        );
                    }
                }
                if let (Some(p), Some(eng)) = (
                    self.player.as_ref(),
                    self.danmaku
                        .as_ref()
                        .filter(|_| self.settings.danmaku_enabled),
                ) {
                    self.danmaku_view
                        .update(eng, p.position(self.frames_in_seg));
                }
            }
        }

        if self.page == Page::Player {
            self.sync_triple();
            let g_player = self.ui.global::<crate::Player>();
            g_player.set_reconnect_text(self.live_recovery.notice(Instant::now()).into());

            g_player.set_liked(self.controls.liked.unwrap_or(false));
            g_player.set_reaction_busy(self.controls.busy);
            if let Some(p) = self.player.as_ref() {
                g_player.set_ptitle(p.title.as_str().into());
                g_player.set_paused(p.paused);
                g_player.set_finished(p.finished);
                g_player.set_speed_text(format!("{}X", p.rate).into());
                // 待提交的跳转使用预览位置
                let pos = self
                    .seek_pending
                    .unwrap_or_else(|| p.position(self.frames_in_seg));
                let dur = p.duration;
                g_player.set_progress(if dur > 0.0 { (pos / dur) as f32 } else { 0.0 });
                let time = if let Some(live) = &self.live {
                    if live.started_at > 0 {
                        model::mmss((crate::net::now_secs() - live.started_at).max(0) as f64)
                    } else {
                        "--:--".into()
                    }
                } else {
                    format!("{}/{}", model::mmss(pos), model::mmss(dur))
                };
                g_player.set_time_text(time.into());
                let est = util::est_w(&p.title, 10.0);
                let clip = if self.live.is_some() {
                    WIDTH as f64 - util::est_w(&g_player.get_watching(), 10.0) - 32.0
                } else {
                    WIDTH as f64 - 12.0
                };
                g_player.set_ptitle_marquee(est > clip);
                g_player.set_ptitle_shift(yoyo_shift(est, clip, self.marq_t));
            }
        }
    }
}
