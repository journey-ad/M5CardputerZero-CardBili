use super::super::*;
use super::{TripleFeedback, TRIPLE_HOLD_DELAY};
use slint::ComponentHandle as _;

#[derive(Default)]
pub(in crate::app) struct Controls {
    pub danmaku_enabled: bool,
    pub liked: Option<bool>,
    pub busy: bool,
    pub like_hold: Option<(Instant, bool)>,
    pub triple: Option<TripleFeedback>,
    pub volume_until: Option<Instant>,
}

impl App {
    pub(in crate::app) fn liked_loaded(&mut self, request: u64, result: Result<bool, String>) {
        if self.page == Page::Player && request == self.play_request {
            self.controls.busy = false;
            match result {
                Ok(liked) => self.controls.liked = Some(liked),
                Err(error) => self.toast(&error.replace('\n', " · ")),
            }
        }
    }

    pub(in crate::app) fn reaction_finished(
        &mut self,
        request: u64,
        result: Result<model::ReactionResult, String>,
    ) {
        if self.page == Page::Player && request == self.play_request {
            self.controls.busy = false;
            match result {
                Ok(result) => {
                    self.finish_triple(result.triple_complete);
                    if let Some(liked) = result.liked {
                        self.controls.liked = Some(liked);
                    }
                    self.toast(&result.message);
                }
                Err(error) => {
                    self.finish_triple(false);
                    self.toast(&error.replace('\n', " · "));
                }
            }
        }
    }

    pub(in crate::app) fn toast(&self, text: &str) {
        let status = self.ui.global::<crate::Status>();
        status.set_bar(text.into());
        status.set_bar_sticky(false);
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
    }

    pub(in crate::app) fn like_release(&mut self, cancel: bool) {
        let status = self.ui.global::<crate::Status>();
        if let Some((since, submitted)) = self.controls.like_hold.take() {
            if !submitted {
                status.set_bar("".into());
                status.set_bar_sticky(false);
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
                info.aid,
                info.bvid.clone(),
                cookies.clone(),
                action,
                self.play_request,
                self.tx.clone(),
            );
        }
    }

    pub(in crate::app) fn show_favorite_choices(&mut self) {
        let player = self.ui.global::<crate::Player>();
        player.set_favorite_loading(self.favorites.loading);
        player.set_favorite_labels(ModelRc::new(VecModel::from(
            self.favorites.folders.iter().map(|folder| folder.title.as_str().into()).collect::<Vec<_>>(),
        )));
        if !self.favorites.error.is_empty() {
            self.toast(&self.favorites.error);
        } else if self.favorites.loaded && self.favorites.folders.is_empty() {
            self.toast(crate::i18n::tr("请先在哔哩哔哩创建收藏夹"));
        }
    }

    pub(in crate::app) fn tick_like(&mut self) {
        let status = self.ui.global::<crate::Status>();
        if self.page != Page::Player {
            return;
        }
        if let Some((since, submitted)) = self.controls.like_hold.as_mut() {
            if !*submitted && since.elapsed() >= TRIPLE_HOLD_DELAY + Duration::from_secs(2) {
                *submitted = true;
                self.controls.triple = Some(TripleFeedback::Pending(Instant::now()));
                status.set_bar("".into());
                status.set_bar_sticky(false);
                self.react(model::VideoAction::Triple);
            }
        }
    }

    pub(in crate::app) fn change_speed(&mut self) {
        if let Some(player) = self.player.as_mut() {
            if let Err(error) = player.cycle_rate() {
                self.toast(&error.replace('\n', " · "));
            }
        }
    }

    pub(in crate::app) fn toggle_danmaku(&mut self) {
        self.controls.danmaku_enabled = !self.controls.danmaku_enabled;
        if self.controls.danmaku_enabled {
            if self.live.is_some() {
                if let Some(player) = &self.player {
                    self.danmaku_view.update_rows(
                        self.live_engine
                            .rows_at(player.position(self.frames_in_seg)),
                    );
                }
            }
            if let (Some(player), Some(engine)) = (&self.player, &self.danmaku) {
                self.danmaku_view
                    .update(engine, player.position(self.frames_in_seg));
            }
        } else {
            self.live_engine = danmaku::LiveEngine::default();
            self.danmaku_view.hide();
        }
    }
}
