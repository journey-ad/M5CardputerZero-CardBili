use super::*;
use slint::ComponentHandle as _;

mod back;
pub(super) use back::BackTarget;

impl App {
    pub(super) fn navigate(&mut self) {
        if self.settings_changed {
            self.settings.list_view = self.view;
            if self.settings.save().is_err() {
                self.ui
                    .global::<crate::Status>()
                    .set_bar(crate::i18n::tr("设置保存失败").into());
                self.ui.global::<crate::Status>().set_bar_sticky(false);
            }
        }
        if self.requested_uploader.is_some() && self.cookies.is_none() {
            self.login_uploader = self.requested_uploader.take();
            self.requested_login = true;
        }
        if self.requested_following && self.cookies.is_none() {
            self.requested_following = false;
            self.login_following = true;
            self.requested_login = true;
        }
        if self.requested_favorites && self.cookies.is_none() {
            self.requested_favorites = false;
            self.login_favorites = true;
            self.requested_login = true;
        }
        if self.requested_watch_later && self.cookies.is_none() {
            self.requested_watch_later = false;
            self.requested_login = true;
        }
        if self.go_back
            && self.page == Page::Detail
            && self.detail_scroll >= self.ui.global::<crate::Detail>().get_viewport_h() as f64
        {
            self.go_back = false;
            self.detail_scroll = 0.0;
            self.ui.global::<crate::Detail>().set_scroll(0.0);
        }
        let navigating = self.requested_favorites
            || self.requested_folder.is_some()
            || self.requested_home_list.is_some()
            || self.requested_about
            || self.requested_following
            || self.requested_play.is_some()
            || self.requested_live.is_some()
            || self.requested_detail.is_some()
            || self.requested_uploader.is_some()
            || self.requested_related.is_some()
            || self.requested_watch_later
            || self.requested_login
            || self.requested_comments.is_some();
        if navigating || self.go_back || self.return_home {
            self.report_history(true, false);
        }
        if navigating {
            self.save_back_target();
        }
        if self.requested_play.is_some() || self.requested_live.is_some() {
            let same_live = self
                .live
                .as_ref()
                .is_some_and(|info| self.requested_live == Some(info.room_id));
            self.controls = playback::Controls {
                danmaku_enabled: if same_live {
                    self.controls.danmaku_enabled
                } else {
                    self.settings.danmaku_enabled
                },
                ..Default::default()
            };
        }
        if navigating || self.go_back || self.return_home {
            self.leave_page();
        }
        self.open_requested_page();
        if self.go_back {
            self.restore_previous_page();
        }
        if self.return_home {
            self.return_to_home();
        }
        crate::cache::pause_images(self.page == Page::Player);
        if self.back_hint {
            self.ui
                .global::<crate::Status>()
                .set_bar(crate::i18n::tr("长按返回首页").into());
            self.ui.global::<crate::Status>().set_bar_sticky(true);
        }
    }

    fn leave_page(&mut self) {
        if let Some(mut previous) = self.player.take() {
            self.gen = self.gen.max(previous.gen);
            previous.stop();
        }
        self.play_request += 1;
        self.comments_request += 1;
        self.controls.like_hold = None;
        self.controls.triple = None;
        let player = self.ui.global::<crate::Player>();
        player.set_triple_phase(0);
        player.set_menu_open(false);
        player.set_track_menu(crate::TrackMenu::Closed);
        player.set_stats_on(false);
        player.set_subtitle_text("".into());
        if !self.back_hint {
            self.ui.global::<crate::Status>().set_bar("".into());
            self.ui.global::<crate::Status>().set_bar_sticky(false);
        }
        self.detail_request += 1;
        self.collection_request += 1;
        self.following_request += 1;
        self.current_play = None;
        self.live_danmaku = None;
        self.live_engine = danmaku::LiveEngine::default();
        self.super_chat = crate::super_chat::State::default();
        self.ui.global::<crate::SuperChat>().set_open(false);
        self.ui.global::<crate::SuperChat>().set_available(false);
        self.ui
            .global::<crate::SuperChat>()
            .set_rows(ModelRc::default());
        self.live = None;
        self.live_recovery = playback::LiveRecovery::default();
        self.ui.global::<crate::Player>().set_live(false);
        self.ui
            .global::<crate::Player>()
            .set_live_audience("".into());
        if self.requested_play.is_none() {
            self.play_start = 0.0;
            self.requested_cid = None;
        }
        self.danmaku = None;
        self.frames_in_seg = 0;
        self.seek_pending = None;
        self.seek_deadline = None;
        self.direction_hold = None;
        self.ui.global::<crate::Player>().set_has_frame(false);
        self.ui.global::<crate::Player>().set_paused(false);
        self.ui.global::<crate::Player>().set_finished(false);
        self.danmaku_view.clear();
    }

    fn open_requested_page(&mut self) {
        if let Some(source) = self.requested_home_list.take() {
            self.page = Page::Home;
            self.mine_detail = source as i32;
            self.sel = 0;
            self.list_scroll = 0.0;
            self.grid_scroll = 0.0;
            self.loading = !self.loaded[source];
            self.home_dirty = true;
            if self.loading {
                self.status = crate::i18n::tr("加载中…").into();
            }
            self.load_home_list(source, false);
        }
        if self.requested_about {
            self.requested_about = false;
            self.page = Page::About;
            self.about_scroll = 0.0;
        }
        if self.requested_favorites {
            self.requested_favorites = false;
            self.page = Page::Favorites;
            self.favorites.dirty = true;
            self.load_favorites(false);
        }
        if self.requested_following {
            self.requested_following = false;
            self.page = Page::Following;
            self.following = following::FollowingState {
                has_more: true,
                dirty: true,
                ..Default::default()
            };
            self.load_following();
        }
        if let Some(aid) = self.requested_comments.take() {
            self.open_comments(aid);
        }
        if self.requested_login {
            self.requested_login = false;
            self.page = Page::Login;
            self.start_login(crate::i18n::tr("正在生成二维码…"));
        }
        if let Some(room_id) = self.requested_live.take() {
            self.live = Some(model::LiveInfo {
                room_id,
                ..Default::default()
            });
            self.page = Page::Player;
            self.sections = sections::Sections::default();
            self.ui.global::<crate::Player>().set_ptitle("".into());
            self.ui.global::<crate::Player>().set_watching("".into());
            self.ui.global::<crate::Player>().set_progress(0.0);
            self.ui.global::<crate::Player>().set_live(true);
            self.ui
                .global::<crate::Player>()
                .set_time_text("--:--".into());
            self.status = crate::i18n::tr("加载中…").into();
            self.status_detail.clear();
            tasks::start_live(
                room_id,
                self.cookies.clone(),
                self.play_request,
                self.tx.clone(),
            );
        }
        if let Some(bvid) = self.requested_play.take() {
            self.live = None;
            self.ui.global::<crate::Player>().set_live(false);
            self.page = Page::Player;
            if self.sections.bvid != bvid {
                self.sections = sections::Sections {
                    loading: true,
                    dirty: true,
                    ..Default::default()
                };
            }
            self.ui.global::<crate::Player>().set_ptitle("".into());
            self.ui.global::<crate::Player>().set_watching("".into());
            self.ui.global::<crate::Player>().set_progress(0.0);
            self.ui
                .global::<crate::Player>()
                .set_time_text("0:00 / 0:00".into());
            self.status = crate::i18n::tr("加载中…").into();
            self.status_detail.clear();
            tasks::start_play(
                bvid,
                self.requested_cid.take(),
                self.cookies.clone(),
                self.play_request,
                self.settings.cdn.clone(),
                self.tx.clone(),
            );
        }
        if let Some((bvid, title)) = self.requested_detail.take() {
            self.page = Page::Detail;
            self.detail_bvid = bvid.clone();
            self.detail_display = None;
            self.sections = sections::Sections::default();
            self.comment_page = comments::Comments::default();
            self.comment_page.loading = true;
            self.comment_page.dirty = true;
            self.detail_scroll = std::env::var("BILI_DSCROLL")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(0.0);
            self.detail_maxv = 0.0;
            show_detail_loading(&self.ui, &title);
            self.ui
                .global::<crate::Detail>()
                .set_bvid(bvid.as_str().into());
            tasks::start_detail(
                bvid,
                self.cookies.clone(),
                self.detail_request,
                self.tx.clone(),
            );
        }
        let collection_source = self
            .requested_uploader
            .take()
            .map(model::VideoSource::Author)
            .or_else(|| {
                self.requested_folder
                    .take()
                    .map(model::VideoSource::Favorites)
            })
            .or_else(|| {
                self.requested_related
                    .take()
                    .map(model::VideoSource::Related)
            })
            .or_else(|| {
                std::mem::take(&mut self.requested_watch_later)
                    .then_some(model::VideoSource::WatchLater)
            });
        if let Some(source) = collection_source {
            self.page = Page::Collection;
            self.collection = CollectionState {
                source,
                pending_page: 1,
                loading: true,
                has_more: true,
                ..Default::default()
            };
            self.sel = 0;
            self.list_scroll = 0.0;
            self.grid_scroll = 0.0;
            self.collection_dirty = true;
            tasks::load_collection(
                self.collection.source.clone(),
                1,
                self.cookies.clone(),
                self.collection_request,
                self.tx.clone(),
            );
            self.load_author_profile();
        }
    }

    fn return_to_home(&mut self) {
        self.close_cdn();
        self.ui.global::<crate::LiveAreas>().set_open(false);
        self.return_home = false;
        self.back_stack.clear();
        self.clear_login_destination();
        self.login_request += 1;
        self.page = Page::Home;
        self.tab = HomeTab::Recommended;
        self.mine_detail = -1;
        self.help = false;
        self.sel = 0;
        self.list_scroll = 0.0;
        self.grid_scroll = 0.0;
        self.home_dirty = true;
        self.loading = !self.loaded[0];
        if self.loading {
            self.load_home_list(0, false);
        }
    }
}
