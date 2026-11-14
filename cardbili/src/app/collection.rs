use slint::ComponentHandle as _;
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
            self.ui.global::<crate::Collection>().set_is_author(false);
            let title = match &self.collection.source {
                model::VideoSource::Related(_) => "相关推荐".to_string(),
                model::VideoSource::Favorites(folder) => {
                    format!("{} · {} 条", folder.title, folder.count)
                }
                model::VideoSource::Author(_) => unreachable!(),
            };
            self.ui.global::<crate::Collection>().set_title(title.into());
            self.ui.global::<crate::Collection>().set_empty(
                if matches!(self.collection.source, model::VideoSource::Favorites(_)) {
                    "暂无收藏视频"
                } else {
                    "暂无相关视频"
                }
                .into(),
            );
            return;
        };
        self.ui.global::<crate::Collection>().set_is_author(true);
        let profile = self.collection.profile.as_ref();
        self.ui.global::<crate::Collection>().set_name(
            profile
                .map(|profile| profile.name.as_str())
                .unwrap_or(&owner.name)
                .into(),
        );
        self.ui.global::<crate::Collection>().set_uid(owner.mid.as_str().into());
        self.ui.global::<crate::Collection>().set_level(
            profile
                .map(|profile| format!("Lv{}", profile.level))
                .unwrap_or_default()
                .into(),
        );
        self.ui.global::<crate::Collection>().set_followers(
            profile
                .map(|profile| format!("粉丝 {}", model::fmt_num(profile.followers)))
                .unwrap_or_default()
                .into(),
        );
        self.ui.global::<crate::Collection>().set_has_avatar(self.collection.avatar.is_some());
        if let Some(avatar) = &self.collection.avatar {
            self.ui.global::<crate::Collection>().set_avatar(avatar.clone());
        }
    }

    pub(super) fn collection_key(&mut self, k: &str) {
        if !self.help && self.video_list_key(k, self.collection.items.len()) {
            return;
        }
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
