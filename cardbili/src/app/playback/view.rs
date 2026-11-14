use slint::ComponentHandle as _;
use super::super::*;

impl App {
    pub(in crate::app) fn sync_playback(&mut self) {
        // --- 应用最新帧 ---
        if let Some((frame_gen, bytes)) = self.latest.take() {
            if self.page == Page::Player
                && self
                    .player
                    .as_ref()
                    .is_some_and(|player| player.gen == frame_gen)
                && bytes.len() == VID_BYTES
            {
                let mut pb = slint::SharedPixelBuffer::<Rgb8Pixel>::new(WIDTH, HEIGHT);
                pb.make_mut_bytes().copy_from_slice(&bytes);
                self.ui.global::<crate::Player>().set_frame(slint::Image::from_rgb8(pb));
                self.ui.global::<crate::Player>().set_has_frame(true);
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
            self.ui.global::<crate::Player>().set_liked(self.controls.liked.unwrap_or(false));
            self.ui.global::<crate::Player>().set_reaction_busy(self.controls.busy);
            if let Some(p) = self.player.as_ref() {
                self.ui.global::<crate::Player>().set_ptitle(p.title.as_str().into());
                self.ui.global::<crate::Player>().set_paused(p.paused);
                self.ui.global::<crate::Player>().set_finished(p.finished);
                self.ui.global::<crate::Player>().set_speed_text(format!("{}X", p.rate).into());
                // 待提交的跳转使用预览位置
                let pos = self
                    .seek_pending
                    .unwrap_or_else(|| p.position(self.frames_in_seg));
                let dur = p.duration;
                self.ui.global::<crate::Player>().set_progress(if dur > 0.0 { (pos / dur) as f32 } else { 0.0 });
                self.ui.global::<crate::Player>().set_time_text(
                    format!("{}/{}", model::mmss(pos), model::mmss(dur))
                        .as_str()
                        .into(),
                );
                let est = util::est_w(&p.title, 10.0);
                let clip = (WIDTH as f64) - 12.0;
                self.ui.global::<crate::Player>().set_ptitle_marquee(est > clip);
                self.ui.global::<crate::Player>().set_ptitle_shift(yoyo_shift(est, clip, self.marq_t));
            }
        }
    }
}
