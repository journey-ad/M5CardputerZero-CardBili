use super::*;

impl App {
    pub(super) fn sync_view(&mut self) {
        // --- 跑马灯计时 ---
        let now = Instant::now();
        self.marq_t += now.duration_since(self.last_tick).as_secs_f64();
        self.last_tick = now;

        // --- 详情页滚动尺寸 ---
        if self.page == Page::Detail && !self.ui.get_detail_loading() {
            self.ui.set_detail_title_shift(yoyo_shift(
                self.ui.get_detail_title_text_width() as f64,
                self.ui.get_detail_title_width() as f64,
                self.marq_t,
            ));
            let m = (self.ui.get_detail_content_h() as f64
                - self.ui.get_detail_viewport_h() as f64)
                .max(0.0);
            if (m - self.detail_maxv).abs() > 0.5 {
                self.detail_maxv = m;
            }
            if std::env::var("BILI_DSEND").is_ok() {
                self.detail_scroll = self.detail_maxv;
                self.ui.set_detail_scroll(self.detail_scroll as f32);
            }
            if self.detail_scroll > self.detail_maxv {
                self.detail_scroll = self.detail_maxv;
                self.ui.set_detail_scroll(self.detail_scroll as f32);
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
                .unwrap_or(true)
        } else {
            true
        };
        let actions = self.menu_actions();
        if self.menu_view != Some(actions) {
            self.ui.set_menu_labels(ModelRc::new(VecModel::from(
                actions
                    .iter()
                    .map(|action| action.map(menu::Action::label).unwrap_or("").into())
                    .collect::<Vec<slint::SharedString>>(),
            )));
            self.menu_view = Some(actions);
        }
        self.ui.set_controls_visible(controls_show);
        self.ui.set_page(self.page);
        self.ui.set_tab(self.tab);
        self.ui.set_sel(self.sel);
        let animate_loading =
            self.page == Page::Player && !self.ui.get_has_frame() && self.status.ends_with('…');
        let status = if animate_loading {
            self.status.trim_end_matches('…')
        } else {
            self.status.as_str()
        };
        self.ui.set_loading_dots(if animate_loading {
            (self.marq_t / 0.4) as i32 % 4
        } else {
            -1
        });
        self.ui.set_status(status.into());
        self.ui
            .set_status_detail(self.status_detail.as_str().into());
        self.ui.set_loading(self.loading);
        self.ui.set_view(self.view);
        self.ui.set_danmaku_enabled(self.settings.danmaku_enabled);
        self.ui.set_danmaku_opacity(self.settings.danmaku_opacity);
        self.ui.set_menu_position(self.settings.menu_position);
        self.ui.set_record_history(self.settings.record_history);
        self.ui.set_debug_log(self.settings.debug_log);
        self.ui
            .set_cdn_current(crate::cdn::index(&self.settings.cdn) as i32);
        self.ui.set_cdn_name(
            crate::cdn::OPTIONS[crate::cdn::index(&self.settings.cdn)]
                .name
                .into(),
        );
        self.ui.set_about_scroll(self.about_scroll as f32);
        self.ui.set_help(self.help);
        self.ui.set_mine_detail(self.mine_detail >= 0);
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
        if self.ui.get_cdn_open() {
            return [
                Some(menu::Action::SpeedTest),
                Some(menu::Action::Select),
                None,
                None,
                Some(menu::Action::Back),
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
