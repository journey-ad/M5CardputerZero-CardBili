use super::*;
use slint::ComponentHandle as _;

impl App {
    pub(super) fn sync_view(&mut self) {
        let detail = self.ui.global::<crate::Detail>();
        let g_player = self.ui.global::<crate::Player>();
        let nav = self.ui.global::<crate::Nav>();
        let g_status = self.ui.global::<crate::Status>();
        let preferences = self.ui.global::<crate::Preferences>();
        // --- 跑马灯计时 ---
        let now = Instant::now();
        self.marq_t += now.duration_since(self.last_tick).as_secs_f64();
        self.last_tick = now;

        // --- 详情页滚动尺寸 ---
        if self.page == Page::Detail && !detail.get_loading() {
            detail.set_title_shift(yoyo_shift(
                detail.get_title_text_width() as f64,
                detail.get_title_width() as f64,
                self.marq_t,
            ));
            let m = (detail.get_content_h() as f64 - detail.get_viewport_h() as f64).max(0.0);
            if (m - self.detail_maxv).abs() > 0.5 {
                self.detail_maxv = m;
            }
            if std::env::var("BILI_DSEND").is_ok() {
                self.detail_scroll = self.detail_maxv;
                detail.set_scroll(self.detail_scroll as f32);
            }
            if self.detail_scroll > self.detail_maxv {
                self.detail_scroll = self.detail_maxv;
                detail.set_scroll(self.detail_scroll as f32);
            }
            sync_thumb(&self.ui, self.detail_scroll, self.detail_maxv);
        }

        // --- 同步 UI ---
        // 播放中 3 秒无操作隐藏控件；暂停、播放结束或正在长按调进度时保持显示
        let controls_show = if self.page == Page::Player {
            self.player
                .as_ref()
                .map(|p| {
                    p.paused
                        || p.finished
                        || self.controls.like_hold.is_some()
                        || self.direction_hold.is_some()
                        || self.last_input.elapsed() < Duration::from_millis(3000)
                })
                .unwrap_or(
                    !g_player.get_has_frame()
                        || self.last_input.elapsed() < Duration::from_millis(3000),
                )
        } else {
            true
        };
        let actions = self.menu_actions();
        if self.menu_view != Some(actions) {
            self.ui
                .global::<crate::Menu>()
                .set_labels(ModelRc::new(VecModel::from(
                    actions
                        .iter()
                        .map(|action| action.map(menu::Action::label).unwrap_or("").into())
                        .collect::<Vec<slint::SharedString>>(),
                )));
            self.menu_view = Some(actions);
        }
        g_player.set_controls_visible(controls_show);
        nav.set_page(self.page);
        nav.set_tab(self.tab);
        nav.set_sel(self.sel);
        let animate_loading =
            self.page == Page::Player && !g_player.get_has_frame() && self.status.ends_with('…');
        let status = if animate_loading {
            self.status.trim_end_matches('…')
        } else {
            self.status.as_str()
        };
        g_status.set_loading_dots(if animate_loading {
            (self.marq_t / 0.4) as i32 % 4
        } else {
            -1
        });
        g_status.set_status(status.into());
        g_status.set_detail(self.status_detail.as_str().into());
        self.ui
            .global::<crate::ListModel>()
            .set_loading(self.loading);
        nav.set_view(self.view);
        self.ui
            .global::<crate::Danmaku>()
            .set_enabled(self.controls.danmaku_enabled);
        preferences.set_danmaku_default(self.settings.danmaku_enabled);
        preferences.set_danmaku_opacity(self.settings.danmaku_opacity);
        preferences.set_menu_position(self.settings.menu_position);
        preferences.set_record_history(self.settings.record_history);
        preferences.set_debug_log(self.settings.debug_log);
        preferences.set_color_emoji(self.settings.color_emoji);
        preferences.set_pixel_optimize(self.settings.pixel_optimize);
        preferences.set_scale_algorithm_name(self.settings.scale_algorithm.label().into());
        self.ui
            .global::<crate::Cdn>()
            .set_current(crate::cdn::index(&self.settings.cdn) as i32);
        preferences.set_cdn_name(
            crate::cdn::OPTIONS[crate::cdn::index(&self.settings.cdn)]
                .name
                .into(),
        );
        self.ui
            .global::<crate::Meta>()
            .set_about_scroll(self.about_scroll as f32);
        nav.set_help(self.help);
        nav.set_mine_detail(self.mine_detail >= 0);
        self.sync_cache();
        self.sync_home();
        self.sync_account();
        self.sync_playback();
        self.sync_comments();
        self.sync_following();
        self.sync_favorites();
        self.sync_sections();
    }
}

impl App {
    pub(super) fn menu_actions(&self) -> [Option<menu::Action>; 5] {
        if self.ui.global::<crate::LiveAreas>().get_open() {
            return [
                Some(menu::Action::Refresh),
                Some(menu::Action::Select),
                None,
                None,
                Some(menu::Action::Back),
            ];
        }
        if self.ui.global::<crate::Cdn>().get_open() {
            return [
                Some(menu::Action::SpeedTest),
                Some(menu::Action::Select),
                None,
                None,
                Some(menu::Action::Back),
            ];
        }
        if self.page == Page::Player && self.live.is_some() {
            let super_chat = self.ui.global::<crate::SuperChat>();
            if super_chat.get_open() {
                return [None, None, None, None, Some(menu::Action::Back)];
            }
            return [
                Some(menu::Action::TogglePlay),
                Some(menu::Action::Refresh),
                super_chat
                    .get_available()
                    .then_some(menu::Action::SuperChat),
                Some(menu::Action::Danmaku),
                None,
            ];
        }
        if self.sections.open {
            return [
                None,
                Some(menu::Action::Play),
                None,
                None,
                Some(menu::Action::Back),
            ];
        }
        let retry = match self.page {
            Page::Comments => !self.comment_page.error.is_empty(),
            Page::Following => !self.following.error.is_empty(),
            Page::Favorites => !self.favorites.error.is_empty(),
            _ => !self.collection.error.is_empty(),
        };
        let mut actions = menu::actions(
            self.page,
            self.tab,
            self.mine_detail,
            self.help,
            self.cookies.is_some(),
            self.settings.menu_position,
            retry,
        );
        if self.page == Page::Detail && self.back_stack.last().is_some_and(|target| {
            matches!(target, BackTarget::Collection(collection) if matches!(collection.source, model::VideoSource::Author(_)))
        }) {
            actions[2] = None;
        }
        if self.page == Page::Detail && self.sections.available() {
            actions[0] = Some(menu::Action::Sections);
        }
        actions
    }
}
