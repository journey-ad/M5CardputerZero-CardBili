use super::super::*;
use super::{TripleFeedback, TRIPLE_HOLD_DELAY};

#[derive(Default)]
pub(in crate::app) struct Controls {
    pub liked: Option<bool>,
    pub busy: bool,
    pub like_hold: Option<(Instant, bool)>,
    pub triple: Option<TripleFeedback>,
}

impl App {
    pub(in crate::app) fn toast(&self, text: &str) {
        self.ui.set_bar(text.into());
        self.ui.set_bar_sticky(false);
    }

    pub(in crate::app) fn refresh_liked(&mut self) {
        self.controls.liked = None;
        self.controls.busy = false;
        if let (Some(info), Some(cookies)) = (&self.current_play, &self.cookies) {
            self.controls.busy = true;
            tasks::playback::fetch_liked(
                info.bvid.clone(),
                cookies.clone(),
                self.play_request,
                self.tx.clone(),
            );
        }
    }

    pub(in crate::app) fn like_press(&mut self) {
        if self.current_play.is_none() {
            return;
        }
        if self.cookies.is_none() {
            self.requested_login = true;
            self.login_return = true;
            return;
        }
        if self.controls.busy {
            return;
        }
        if self.controls.liked.is_none() {
            self.refresh_liked();
            return;
        }
        self.controls.triple = None;
        self.controls.like_hold = Some((Instant::now(), false));
        self.ui.set_bar("长按一键三连".into());
        self.ui.set_bar_sticky(true);
    }

    pub(in crate::app) fn like_release(&mut self, cancel: bool) {
        if let Some((since, submitted)) = self.controls.like_hold.take() {
            if !submitted {
                self.ui.set_bar("".into());
                self.ui.set_bar_sticky(false);
                if !cancel && since.elapsed() < TRIPLE_HOLD_DELAY {
                    self.react(model::VideoAction::Like(
                        !self.controls.liked.unwrap_or(false),
                    ));
                }
            }
        }
    }

    pub(in crate::app) fn react(&mut self, action: model::VideoAction) {
        if self.controls.busy {
            return;
        }
        if let (Some(info), Some(cookies)) = (&self.current_play, &self.cookies) {
            self.controls.busy = true;
            tasks::playback::react(
                info.bvid.clone(),
                cookies.clone(),
                action,
                self.play_request,
                self.tx.clone(),
            );
        }
    }

    pub(in crate::app) fn tick_like(&mut self) {
        if self.page != 1 {
            return;
        }
        if let Some((since, submitted)) = self.controls.like_hold.as_mut() {
            if !*submitted && since.elapsed() >= Duration::from_secs(2) {
                *submitted = true;
                self.controls.triple = Some(TripleFeedback::Pending(Instant::now()));
                self.ui.set_bar("".into());
                self.ui.set_bar_sticky(false);
                self.react(model::VideoAction::Triple);
            }
        }
    }

    pub(in crate::app) fn change_speed(&mut self) {
        if let Some(player) = self.player.as_mut() {
            let position = self
                .seek_pending
                .take()
                .unwrap_or_else(|| player.position(self.frames_in_seg));
            self.seek_deadline = None;
            player.cycle_rate(position);
            self.frames_in_seg = 0;
            self.ui.set_has_frame(false);
            self.status = "加载中…".into();
        }
    }

    pub(in crate::app) fn toggle_danmaku(&mut self) {
        self.settings.danmaku_enabled = !self.settings.danmaku_enabled;
        self.settings_changed = true;
        if self.settings.danmaku_enabled {
            if let (Some(player), Some(engine)) = (&self.player, &self.danmaku) {
                render_danmaku(
                    &self.ui,
                    engine,
                    player.position(self.frames_in_seg),
                    &mut self.dm_cache,
                );
            }
        } else {
            self.ui
                .set_dm_rows(ModelRc::new(VecModel::from(Vec::<crate::DmRow>::new())));
        }
    }
}
