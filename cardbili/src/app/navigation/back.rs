use super::super::*;
use slint::ComponentHandle as _;

pub(in crate::app) enum BackTarget {
    Home {
        tab: HomeTab,
        mine_detail: i32,
        selected_bvid: Option<String>,
        selection: i32,
        list_scroll: f64,
        grid_scroll: f64,
    },
    Detail {
        bvid: String,
        display: Option<DetailDisplay>,
        comments: comments::Comments,
        sections: sections::Sections,
        scroll: f64,
    },
    Collection(CollectionState),
    Following(following::FollowingState),
    Favorites {
        selected_id: Option<i64>,
        selection: usize,
        first: usize,
    },
    Play {
        info: model::PlayInfo,
        position: f64,
        paused: bool,
        rate: f64,
        engine: Option<danmaku::Engine>,
        danmaku_enabled: bool,
        sections: sections::Sections,
        media: playback::Media,
        subtitle_selected: i32,
        audio_selected: i32,
    },
}

impl App {
    pub(super) fn save_back_target(&mut self) {
        let target = match self.page {
            Page::Home => Some(BackTarget::Home {
                tab: self.tab,
                mine_detail: self.mine_detail,
                selected_bvid: list_source(self.tab, self.mine_detail)
                    .and_then(|source| self.lists[source].get(self.sel as usize))
                    .map(|item| item.bvid.clone()),
                selection: self.sel,
                list_scroll: self.list_scroll,
                grid_scroll: self.grid_scroll,
            }),
            Page::Detail => Some(BackTarget::Detail {
                bvid: self.detail_bvid.clone(),
                display: self.detail_display.clone(),
                comments: std::mem::take(&mut self.comment_page),
                sections: self.sections.clone(),
                scroll: self.detail_scroll,
            }),
            Page::Favorites => Some(BackTarget::Favorites {
                selected_id: self
                    .favorites
                    .folders
                    .get(self.favorites.selection)
                    .map(|folder| folder.id),
                selection: self.favorites.selection,
                first: self.favorites.first,
            }),
            Page::Following => Some(BackTarget::Following(std::mem::take(&mut self.following))),
            Page::Collection => {
                self.collection.selection = self.sel;
                self.collection.list_scroll = self.list_scroll;
                self.collection.grid_scroll = self.grid_scroll;
                Some(BackTarget::Collection(std::mem::take(&mut self.collection)))
            }
            Page::Player if self.requested_play.is_none() => {
                self.current_play.take().map(|info| BackTarget::Play {
                    info,
                    position: self
                        .player
                        .as_ref()
                        .map(|player| player.position(self.frames_in_seg))
                        .unwrap_or(0.0),
                    paused: self.player.as_ref().is_some_and(|player| player.paused),
                    rate: self
                        .player
                        .as_ref()
                        .map(|player| player.rate)
                        .unwrap_or(1.0),
                    engine: self.danmaku.take(),
                    danmaku_enabled: self.controls.danmaku_enabled,
                    sections: self.sections.clone(),
                    media: std::mem::take(&mut self.media),
                    subtitle_selected: self.ui.global::<crate::Player>().get_subtitle_selected(),
                    audio_selected: self.ui.global::<crate::Player>().get_audio_selected(),
                })
            }
            _ => None,
        };
        if let Some(target) = target {
            self.back_stack.push(target);
        }
    }

    pub(super) fn restore_previous_page(&mut self) {
        match self.back_stack.pop() {
            Some(BackTarget::Home {
                tab: previous_tab,
                mine_detail,
                selected_bvid,
                selection,
                list_scroll: scroll_y,
                grid_scroll: scroll,
            }) => {
                self.page = Page::Home;
                self.tab = previous_tab;
                self.mine_detail = mine_detail;
                self.sel = list_source(previous_tab, mine_detail)
                    .and_then(|source| {
                        self.lists[source]
                            .iter()
                            .position(|item| Some(&item.bvid) == selected_bvid.as_ref())
                    })
                    .map(|index| index as i32)
                    .unwrap_or(selection);
                self.list_scroll = scroll_y;
                self.grid_scroll = scroll;
                self.home_dirty = true;
                self.status.clear();
                self.status_detail.clear();
                // 返回尚未加载的列表时自动请求数据
                if let Some(source) = list_source(previous_tab, mine_detail) {
                    if !self.loaded[source] {
                        self.loading = true;
                        self.status = crate::i18n::tr("加载中…").into();
                        self.load_home_list(source, false);
                    }
                }
            }
            Some(BackTarget::Detail {
                bvid,
                display,
                comments,
                sections,
                scroll,
            }) => {
                self.page = Page::Detail;
                self.detail_bvid = bvid;
                self.detail_scroll = scroll;
                self.detail_display = display;
                self.comment_page = comments;
                self.sections = sections;
                if self.sections.loading {
                    self.load_chapters();
                }
                let resume_comments = self.comment_page.loading;
                self.comment_page.loading = false;
                self.comment_page.dirty = true;
                if resume_comments && self.comment_page.aid > 0 {
                    self.load_comments();
                }
                self.marq_t = 0.0;
                self.ui
                    .global::<crate::Detail>()
                    .set_bvid(self.detail_bvid.as_str().into());
                if let Some(display) = &self.detail_display {
                    show_detail(&self.ui, display);
                } else {
                    show_detail_loading(&self.ui, "");
                    tasks::start_detail(
                        self.detail_bvid.clone(),
                        self.cookies.clone(),
                        self.detail_request,
                        self.tx.clone(),
                    );
                }
                self.ui
                    .global::<crate::Detail>()
                    .set_scroll(self.detail_scroll as f32);
            }
            Some(BackTarget::Favorites {
                selected_id,
                selection,
                first,
            }) => {
                self.page = Page::Favorites;
                self.favorites.selection = self
                    .favorites
                    .folders
                    .iter()
                    .position(|folder| Some(folder.id) == selected_id)
                    .unwrap_or(selection);
                self.favorites.first = first;
                self.favorites.dirty = true;
            }
            Some(BackTarget::Following(previous)) => {
                self.page = Page::Following;
                self.following = previous;
                self.following.loading = false;
                self.following.dirty = true;
                let missing = self
                    .following
                    .items
                    .iter()
                    .filter_map(|entry| match entry {
                        following::FollowingEntry::User(user)
                            if !self.following.avatars.contains_key(&user.owner.mid) =>
                        {
                            Some(user.clone())
                        }
                        _ => None,
                    })
                    .collect();
                tasks::load_following_avatars(missing, self.following_request, self.tx.clone());
                if self.following.items.is_empty() {
                    self.load_following();
                }
            }
            Some(BackTarget::Collection(previous)) => {
                self.page = Page::Collection;
                self.collection = previous;
                self.sel = self.collection.selection;
                self.list_scroll = self.collection.list_scroll;
                self.grid_scroll = self.collection.grid_scroll;
                self.collection.loading = false;
                self.collection_dirty = true;
                let missing_covers = self
                    .collection
                    .items
                    .iter()
                    .zip(&self.collection.covers)
                    .filter(|(_, cover)| cover.is_none())
                    .map(|(item, _)| item.clone())
                    .collect::<Vec<_>>();
                if !missing_covers.is_empty() {
                    tasks::spawn_covers(
                        crate::app::home::COLLECTION_SOURCE,
                        missing_covers,
                        self.tx.clone(),
                    );
                }
                if !matches!(self.collection.source, model::VideoSource::Related(_)) {
                    self.collection.loading = self.collection.items.is_empty();
                    self.collection.pending_page = 1;
                    tasks::load_collection(
                        self.collection.source.clone(),
                        1,
                        self.cookies.clone(),
                        self.collection_request,
                        self.tx.clone(),
                    );
                }
                if self.collection.profile.is_none() {
                    self.load_author_profile();
                }
            }
            Some(BackTarget::Play {
                info,
                position,
                paused,
                rate,
                engine,
                danmaku_enabled,
                sections,
                media,
                subtitle_selected,
                audio_selected,
            }) => {
                self.page = Page::Player;
                self.controls.danmaku_enabled = danmaku_enabled;
                self.sections = sections;
                if self.sections.loading {
                    self.load_chapters();
                }
                self.ui.global::<crate::Player>().set_watching("".into());
                tasks::playback::watching(
                    info.bvid.clone(),
                    info.cid,
                    self.cookies.clone(),
                    self.play_request,
                    self.tx.clone(),
                );
                self.gen += 1;
                self.player = Some(player::Player::start(
                    &info,
                    self.cookies.clone().unwrap_or_default(),
                    self.gen,
                    self.tx.clone(),
                    &self.settings,
                    player::PlaybackState {
                        position,
                        paused,
                        rate,
                    },
                ));
                self.danmaku = engine;
                if self.danmaku.is_none() {
                    tasks::start_danmaku(info.cid, self.cookies.clone(), self.gen, self.tx.clone());
                }
                self.current_play = Some(info);
                self.media = media;
                self.restore_media(subtitle_selected, audio_selected);
                self.refresh_liked();
                self.status = crate::i18n::tr("加载中…").into();
            }
            None => self.page = Page::Home,
        }
    }
}
