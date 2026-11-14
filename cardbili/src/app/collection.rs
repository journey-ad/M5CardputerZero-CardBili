use super::*;
use slint::ComponentHandle as _;

#[derive(Default)]
pub(super) struct CollectionState {
    pub(super) source: model::VideoSource,
    pub(super) profile: Option<model::AuthorProfile>,
    pub(super) avatar: Option<slint::Image>,
    pub(super) items: Vec<model::Item>,
    pub(super) covers: Vec<Option<slint::Image>>,
    pub(super) page: u32,
    pub(super) pending_page: u32,
    pub(super) first_page_size: usize,
    pub(super) has_more: bool,
    pub(super) loading: bool,
    pub(super) error: String,
    pub(super) selection: i32,
    pub(super) list_scroll: f64,
    pub(super) grid_scroll: f64,
}

impl App {
    pub(super) fn author_profile_loaded(
        &mut self,
        request: u64,
        result: Result<(model::AuthorProfile, Option<crate::cache::Pixels>), String>,
    ) {
        if self.page != Page::Collection || request != self.collection_request {
            return;
        }
        match result {
            Ok((profile, raw)) => {
                self.collection.avatar =
                    raw.map(|(w, h, pixels)| util::raw_to_image(w, h, &pixels));
                self.collection.profile = Some(profile);
            }
            Err(error) => eprintln!("{error}"),
        }
    }

    pub(super) fn collection_loaded(
        &mut self,
        request: u64,
        loaded_page: u32,
        result: Result<model::VideoPage, String>,
    ) {
        if self.page != Page::Collection || request != self.collection_request {
            return;
        }
        if loaded_page == self.collection.pending_page {
            self.collection.loading = false;
        }
        match result {
            Ok(videos) => {
                if let (model::VideoSource::Favorites(folder), Some(total)) =
                    (&mut self.collection.source, videos.total)
                {
                    folder.count = total;
                }
                if loaded_page == 1 {
                    self.sel = refresh_first_page(
                        &mut self.collection.items,
                        &mut self.collection.covers,
                        &mut self.collection.first_page_size,
                        videos.items,
                        self.sel,
                    );
                } else {
                    let mut known: HashSet<String> = self
                        .collection
                        .items
                        .iter()
                        .map(|item| item.bvid.clone())
                        .collect();
                    self.collection.items.extend(
                        videos
                            .items
                            .into_iter()
                            .filter(|item| known.insert(item.bvid.clone())),
                    );
                    self.collection
                        .covers
                        .resize_with(self.collection.items.len(), || None);
                }
                let missing = self
                    .collection
                    .items
                    .iter()
                    .zip(&self.collection.covers)
                    .filter(|(_, cover)| cover.is_none())
                    .map(|(item, _)| item.clone())
                    .collect();
                tasks::spawn_covers(5, missing, self.tx.clone());
                if loaded_page >= self.collection.page {
                    self.collection.page = loaded_page;
                    self.collection.has_more = videos.has_more;
                }
                self.collection.error.clear();
            }
            Err(error) => {
                self.collection.error = error;
            }
        }
        self.collection_dirty = true;
    }

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
        let g_collection = self.ui.global::<crate::Collection>();
        let model::VideoSource::Author(owner) = &self.collection.source else {
            g_collection.set_is_author(false);
            let title = match &self.collection.source {
                model::VideoSource::Related(_) => "相关推荐".to_string(),
                model::VideoSource::Favorites(folder) => {
                    format!("{} · {} 条", folder.title, folder.count)
                }
                model::VideoSource::Author(_) => unreachable!(),
            };
            g_collection.set_title(title.into());
            g_collection.set_empty(
                if matches!(self.collection.source, model::VideoSource::Favorites(_)) {
                    "暂无收藏视频"
                } else {
                    "暂无相关视频"
                }
                .into(),
            );
            return;
        };
        g_collection.set_is_author(true);
        let profile = self.collection.profile.as_ref();
        g_collection.set_name(
            profile
                .map(|profile| profile.name.as_str())
                .unwrap_or(&owner.name)
                .into(),
        );
        g_collection.set_uid(owner.mid.as_str().into());
        g_collection.set_level(
            profile
                .map(|profile| format!("Lv{}", profile.level))
                .unwrap_or_default()
                .into(),
        );
        g_collection.set_followers(
            profile
                .map(|profile| format!("粉丝 {}", model::fmt_num(profile.followers)))
                .unwrap_or_default()
                .into(),
        );
        g_collection.set_has_avatar(self.collection.avatar.is_some());
        if let Some(avatar) = &self.collection.avatar {
            g_collection.set_avatar(avatar.clone());
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
