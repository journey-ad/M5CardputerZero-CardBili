use super::*;

impl App {
    pub(super) fn load_author_profile(&self) {
        if let model::VideoSource::Author(owner) = &self.collection.source {
            tasks::load_author_profile(
                owner.mid.clone(),
                self.cookies.clone(),
                self.collection_request,
                self.tx.clone(),
            );
        }
    }

    pub(super) fn sync_collection_header(&self) {
        let model::VideoSource::Author(owner) = &self.collection.source else {
            self.ui.set_collection_is_author(false);
            let title = match &self.collection.source {
                model::VideoSource::Related(_) => "相关推荐".to_string(),
                model::VideoSource::Favorites(folder) => {
                    format!("{} · {} 条", folder.title, folder.count)
                }
                model::VideoSource::Author(_) => unreachable!(),
            };
            self.ui.set_collection_title(title.into());
            self.ui.set_collection_empty(
                if matches!(self.collection.source, model::VideoSource::Favorites(_)) {
                    "暂无收藏视频"
                } else {
                    "暂无相关视频"
                }
                .into(),
            );
            return;
        };
        self.ui.set_collection_is_author(true);
        let profile = self.collection.profile.as_ref();
        self.ui.set_collection_name(
            profile
                .map(|profile| profile.name.as_str())
                .unwrap_or(&owner.name)
                .into(),
        );
        self.ui.set_collection_uid(owner.mid.as_str().into());
        self.ui.set_collection_level(
            profile
                .map(|profile| format!("Lv{}", profile.level))
                .unwrap_or_default()
                .into(),
        );
        self.ui.set_collection_followers(
            profile
                .map(|profile| format!("粉丝 {}", model::fmt_num(profile.followers)))
                .unwrap_or_default()
                .into(),
        );
        self.ui
            .set_collection_has_avatar(self.collection.avatar.is_some());
        if let Some(avatar) = &self.collection.avatar {
            self.ui.set_collection_avatar(avatar.clone());
        }
    }

    pub(super) fn page_key(&mut self, k: &str) {
        if self.page == 8 {
            self.favorites_key(k);
        } else if self.page == 7 {
            self.following_key(k);
        } else if self.page == 6 {
            self.comments_key(k);
        } else if self.page == 2 {
            match k {
                "ESC" | "BACKSPACE" => {
                    self.login_uploader = None;
                    self.login_following = false;
                    self.login_favorites = false;
                    self.login_return = false;
                    self.go_back = true;
                }
                "r" => {
                    self.ui.set_has_qr(false);
                    self.ui.set_qr_status("正在刷新二维码…".into());
                    self.login_request += 1;
                    tasks::start_login(self.login_request, self.tx.clone());
                }
                _ => {}
            }
        } else if self.page == 3 {
            match k {
                "ENTER" | "SPACE" => {
                    if !self.detail_bvid.is_empty() {
                        self.status = "正在加载视频…".into();
                        self.status_detail.clear();
                        self.requested_play = Some(self.detail_bvid.clone());
                    }
                }
                "u" => {
                    self.requested_uploader = self
                        .detail_display
                        .as_ref()
                        .map(|display| display.data.owner.clone())
                        .filter(|owner| !owner.mid.is_empty());
                }
                "r" => {
                    if !self.detail_bvid.is_empty() {
                        self.requested_related = Some(self.detail_bvid.clone());
                    }
                }
                "ESC" | "BACKSPACE" => {
                    self.go_back = true;
                }
                _ => {}
            }
        } else if self.page == 4 {
            let limit = (self.ui.get_about_content_h() as f64 - 150.0).max(0.0);
            match k {
                "UP" | "f" | "w" | "PGUP" => {
                    self.about_scroll = (self.about_scroll - 20.0).max(0.0)
                }
                "DOWN" | "x" | "s" | "PGDN" => {
                    self.about_scroll = (self.about_scroll + 20.0).min(limit)
                }
                "ESC" | "BACKSPACE" | "ENTER" => self.go_back = true,
                _ => {}
            }
        } else if self.page == 5 {
            match k {
                "ENTER" => {
                    self.requested_play = self
                        .collection
                        .items
                        .get(self.sel as usize)
                        .map(|item| item.bvid.clone())
                }
                "SPACE" => {
                    self.requested_detail = self
                        .collection
                        .items
                        .get(self.sel as usize)
                        .map(|item| (item.bvid.clone(), item.title.clone()))
                }
                "r" => {
                    self.collection_request += 1;
                    self.collection.loading = true;
                    self.collection.error.clear();
                    self.collection.pending_page = if self.collection.items.is_empty() {
                        1
                    } else {
                        self.collection.page + 1
                    };
                    tasks::load_collection(
                        self.collection.source.clone(),
                        self.collection.pending_page,
                        self.cookies.clone(),
                        self.collection_request,
                        self.tx.clone(),
                    );
                    if self.collection.profile.is_none() {
                        self.load_author_profile();
                    }
                }
                "ESC" | "BACKSPACE" => self.go_back = true,
                _ => {}
            }
        }
    }
}
