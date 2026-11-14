use super::*;

#[derive(Clone)]
pub(super) struct DetailDisplay {
    pub(super) data: model::Detail,
    pub(super) cover: Option<slint::Image>,
}

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
    pub(super) first_row: usize,
    pub(super) grid_scroll: f64,
}

pub(super) enum BackTarget {
    Home {
        tab: HomeTab,
        mine_detail: i32,
        selected_bvid: Option<String>,
        selection: i32,
        first_row: usize,
        grid_scroll: f64,
    },
    Detail {
        bvid: String,
        display: Option<DetailDisplay>,
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
    },
}

impl App {
    pub(super) fn navigate(&mut self) {
        if self.settings_changed {
            self.settings.list_view = self.view;
            if let Err(error) = self.settings.save() {
                eprintln!("设置保存失败: {error}");
                self.ui.set_bar("设置保存失败".into());
                self.ui.set_bar_sticky(false);
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
        let navigating = self.requested_favorites
            || self.requested_folder.is_some()
            || self.requested_home_list.is_some()
            || self.requested_about
            || self.requested_following
            || self.requested_play.is_some()
            || self.requested_detail.is_some()
            || self.requested_uploader.is_some()
            || self.requested_related.is_some()
            || self.requested_login
            || self.requested_comments.is_some();
        if navigating || self.go_back || self.return_home {
            self.report_history(true, false);
        }
        if navigating {
            let target = match self.page {
                Page::Home => Some(BackTarget::Home {
                    tab: self.tab,
                    mine_detail: self.mine_detail,
                    selected_bvid: list_source(self.tab, self.mine_detail)
                        .and_then(|source| self.lists[source].get(self.sel as usize))
                        .map(|item| item.bvid.clone()),
                    selection: self.sel,
                    first_row: self.first_row,
                    grid_scroll: self.grid_scroll,
                }),
                Page::Detail => Some(BackTarget::Detail {
                    bvid: self.detail_bvid.clone(),
                    display: self.detail_display.clone(),
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
                    self.collection.first_row = self.first_row;
                    self.collection.grid_scroll = self.grid_scroll;
                    Some(BackTarget::Collection(std::mem::take(&mut self.collection)))
                }
                Page::Player => self.current_play.take().map(|info| BackTarget::Play {
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
                }),
                _ => None,
            };
            if let Some(target) = target {
                self.back_stack.push(target);
            }
        }
        if navigating || self.go_back || self.return_home {
            if let Some(mut previous) = self.player.take() {
                self.gen = self.gen.max(previous.gen);
                previous.stop();
            }
            self.play_request += 1;
            self.comments_request += 1;
            self.controls.like_hold = None;
            self.controls.triple = None;
            self.ui.set_triple_phase(0);
            self.ui.set_bar("".into());
            self.ui.set_bar_sticky(false);
            self.detail_request += 1;
            self.collection_request += 1;
            self.following_request += 1;
            self.current_play = None;
            self.danmaku = None;
            self.frames_in_seg = 0;
            self.seek_pending = None;
            self.seek_deadline = None;
            self.direction_hold = None;
            self.scroll_tap = None;
            self.ui.set_has_frame(false);
            self.ui.set_paused(false);
            self.danmaku_view.clear();
        }
        if let Some(source) = self.requested_home_list.take() {
            self.page = Page::Home;
            self.mine_detail = source as i32;
            self.sel = 0;
            self.first_row = 0;
            self.grid_scroll = 0.0;
            self.loading = !self.loaded[source];
            self.home_dirty = true;
            if self.loading {
                self.status = "加载中…".into();
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
            self.ui.set_has_qr(false);
            self.ui.set_qr_status("正在生成二维码…".into());
            self.login_request += 1;
            tasks::start_login(self.login_request, self.tx.clone());
        }
        if let Some(bvid) = self.requested_play.take() {
            self.controls = playback::Controls::default();
            self.page = Page::Player;
            self.ui.set_ptitle("".into());
            self.ui.set_progress(0.0);
            self.ui.set_time_text("0:00 / 0:00".into());
            self.status = "加载中…".into();
            self.status_detail.clear();
            tasks::start_play(
                bvid,
                self.cookies.clone(),
                self.play_request,
                self.tx.clone(),
            );
        }
        if let Some((bvid, title)) = self.requested_detail.take() {
            self.page = Page::Detail;
            self.detail_bvid = bvid.clone();
            self.detail_display = None;
            self.detail_scroll = std::env::var("BILI_DSCROLL")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(0.0);
            self.detail_maxv = 0.0;
            show_detail_loading(&self.ui, &title);
            self.ui.set_detail_bvid(bvid.as_str().into());
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
            self.first_row = 0;
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
        if self.go_back {
            match self.back_stack.pop() {
                Some(BackTarget::Home {
                    tab: previous_tab,
                    mine_detail,
                    selected_bvid,
                    selection,
                    first_row: first,
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
                    self.first_row = first;
                    self.grid_scroll = scroll;
                    self.home_dirty = true;
                    self.status.clear();
                    self.status_detail.clear();
                }
                Some(BackTarget::Detail {
                    bvid,
                    display,
                    scroll,
                }) => {
                    self.page = Page::Detail;
                    self.detail_bvid = bvid;
                    self.detail_scroll = scroll;
                    self.detail_display = display;
                    self.marq_t = 0.0;
                    self.ui.set_detail_bvid(self.detail_bvid.as_str().into());
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
                    self.ui.set_detail_scroll(self.detail_scroll as f32);
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
                        .filter(|user| !self.following.avatars.contains_key(&user.owner.mid))
                        .cloned()
                        .collect();
                    tasks::load_following_avatars(missing, self.following_request, self.tx.clone());
                    self.following.pending_page = 1;
                    tasks::load_following(
                        self.mid.clone(),
                        1,
                        self.cookies.clone().unwrap_or_default(),
                        self.following_request,
                        self.tx.clone(),
                    );
                }
                Some(BackTarget::Collection(previous)) => {
                    self.page = Page::Collection;
                    self.collection = previous;
                    self.sel = self.collection.selection;
                    self.first_row = self.collection.first_row;
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
                        tasks::spawn_covers(5, missing_covers, self.tx.clone());
                    }
                    self.collection.loading = self.collection.items.is_empty();
                    self.collection.pending_page = 1;
                    tasks::load_collection(
                        self.collection.source.clone(),
                        1,
                        self.cookies.clone(),
                        self.collection_request,
                        self.tx.clone(),
                    );
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
                }) => {
                    self.page = Page::Player;
                    self.gen += 1;
                    self.player = Some(player::Player::start(
                        &info,
                        self.cookies.clone().unwrap_or_default(),
                        self.gen,
                        self.tx.clone(),
                        player::PlaybackState {
                            position,
                            paused,
                            rate,
                        },
                    ));
                    self.danmaku = engine;
                    if self.danmaku.is_none() {
                        tasks::start_danmaku(
                            info.cid,
                            self.cookies.clone(),
                            self.gen,
                            self.tx.clone(),
                        );
                    }
                    self.current_play = Some(info);
                    self.refresh_liked();
                    self.status = "加载中…".into();
                }
                None => self.page = Page::Home,
            }
        }
        if self.return_home {
            self.return_home = false;
            self.back_stack.clear();
            self.login_uploader = None;
            self.login_following = false;
            self.login_favorites = false;
            self.login_return = false;
            self.login_request += 1;
            self.page = Page::Home;
            self.tab = HomeTab::Recommended;
            self.mine_detail = -1;
            self.help = false;
            self.sel = 0;
            self.first_row = 0;
            self.grid_scroll = 0.0;
            self.home_dirty = true;
            self.loading = !self.loaded[0];
            if self.loading {
                self.load_home_list(0, false);
            }
        }
        if std::mem::take(&mut self.back_hint) {
            self.toast("长按返回首页");
        }
    }
}
