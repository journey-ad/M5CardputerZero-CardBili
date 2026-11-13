use super::*;

impl App {
    pub(super) fn sync_view(&mut self) {
        // --- 跑马灯计时 ---
        let now = Instant::now();
        self.marq_t += now.duration_since(self.last_tick).as_secs_f64();
        self.last_tick = now;

        // --- 详情页滚动尺寸 ---
        if self.page == 3 && !self.ui.get_detail_loading() {
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
        // 播放中 3 秒无操作隐藏控件；暂停或正在长按调进度时保持显示
        let controls_show = if self.page == 1 {
            self.player
                .as_ref()
                .map(|p| {
                    p.paused
                        || self.controls.like_hold.is_some()
                        || self.left_hold.is_some()
                        || self.right_hold.is_some()
                        || self.last_input.elapsed() < Duration::from_millis(3000)
                })
                .unwrap_or(true)
        } else {
            true
        };
        let actions = menu::actions(
            self.page,
            self.tab,
            self.mine_detail,
            self.help,
            self.cookies.is_some(),
            self.settings.menu_position,
            if self.page == 6 {
                !self.comment_page.error.is_empty()
            } else {
                !self.uploader.error.is_empty()
            },
        );
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
        self.ui.set_tab(self.tab as i32);
        self.ui.set_sel(self.sel);
        self.ui.set_status(self.status.as_str().into());
        self.ui
            .set_status_detail(self.status_detail.as_str().into());
        self.ui.set_loading(self.loading);
        self.ui.set_view(self.view);
        self.ui.set_danmaku_enabled(self.settings.danmaku_enabled);
        self.ui.set_danmaku_opacity(self.settings.danmaku_opacity);
        self.ui.set_menu_position(self.settings.menu_position);
        self.ui.set_about_scroll(self.about_scroll as f32);
        self.ui.set_help(self.help);
        self.ui.set_mine_detail(self.mine_detail >= 0);
        self.sync_home();
        self.sync_account();
        self.sync_playback();
        self.sync_comments();
    }
}
