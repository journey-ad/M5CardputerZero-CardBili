use super::super::*;
use slint::ComponentHandle as _;

// 播放会话内复用标题宽度、倍速和秒级时间显示
#[derive(Default)]
pub(in crate::app) struct PlaybackDisplay {
    generation: Option<u64>,
    title_width: f64,
    rate: Option<f64>,
    // 总时长 -1 表示直播，当前时间 -1 表示开播时间未知
    time: Option<(i64, i64)>,
}

impl App {
    pub(in crate::app) fn sync_playback(&mut self) {
        let position = self
            .player
            .as_ref()
            .map(|player| player.position(self.frames_in_seg))
            .unwrap_or_default();
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
                    self.danmaku_view
                        .update_rows(self.live_engine.rows_at(position));
                }
                if let Some(engine) = self
                    .danmaku
                    .as_ref()
                    .filter(|_| self.settings.danmaku_enabled)
                {
                    self.danmaku_view.update(engine, position);
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
                let display = &mut self.playback_display;
                if display.generation != Some(p.gen) {
                    *display = PlaybackDisplay {
                        generation: Some(p.gen),
                        title_width: util::est_w(&p.title, 10.0),
                        ..Default::default()
                    };
                    g_player.set_ptitle(p.title.as_str().into());
                }
                g_player.set_paused(p.paused);
                g_player.set_finished(p.finished);
                // 控件显示时更新进度、时间和标题滚动位置
                if !g_player.get_controls_visible() {
                    return;
                }
                if display.rate != Some(p.rate) {
                    g_player.set_speed_text(format!("{}X", p.rate).into());
                    display.rate = Some(p.rate);
                }
                // 待提交的跳转使用预览位置
                let pos = self.seek_pending.unwrap_or(position);
                let dur = p.duration;
                g_player.set_progress(if dur > 0.0 { (pos / dur) as f32 } else { 0.0 });
                let time = if let Some(live) = &self.live {
                    let elapsed = if live.started_at > 0 {
                        (crate::net::now_secs() - live.started_at).max(0)
                    } else {
                        -1
                    };
                    (elapsed, -1)
                } else {
                    (pos.max(0.0) as i64, dur.max(0.0) as i64)
                };
                if display.time != Some(time) {
                    let text = if time.0 < 0 {
                        "--:--".into()
                    } else if time.1 < 0 {
                        model::mmss(time.0 as f64)
                    } else {
                        format!("{}/{}", model::mmss(time.0 as f64), model::mmss(time.1 as f64))
                    };
                    g_player.set_time_text(text.into());
                    display.time = Some(time);
                }
                let est = display.title_width;
                let clip = if self.live.is_some() {
                    let audience = g_player.get_live_audience();
                    let audience_width = if audience.is_empty() {
                        0.0
                    } else {
                        util::est_w(&audience, 10.0) + 21.0
                    };
                    WIDTH as f64 - util::est_w(&g_player.get_watching(), 10.0) - 32.0 - audience_width
                } else {
                    WIDTH as f64 - 12.0
                };
                g_player.set_ptitle_marquee(est > clip);
                g_player.set_ptitle_shift(yoyo_shift(est, clip, self.marq_t));
            }
        }
    }
}
