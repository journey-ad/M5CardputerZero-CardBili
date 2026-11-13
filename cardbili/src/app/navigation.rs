use super::*;

#[derive(Clone)]
pub(super) struct DetailDisplay {
    pub(super) data: model::Detail,
    pub(super) cover: Option<slint::Image>,
}

#[derive(Default)]
pub(super) struct UploaderState {
    pub(super) owner: model::Owner,
    pub(super) items: Vec<model::Item>,
    pub(super) covers: Vec<Option<slint::Image>>,
    pub(super) page: u32,
    pub(super) has_more: bool,
    pub(super) loading: bool,
    pub(super) error: String,
    pub(super) selection: i32,
    pub(super) first_row: usize,
    pub(super) grid_scroll: f64,
}

pub(super) enum BackTarget {
    Home {
        tab: usize,
        selection: i32,
        first_row: usize,
        grid_scroll: f64,
    },
    Detail {
        bvid: String,
        display: Option<DetailDisplay>,
        from_uploader: bool,
        scroll: f64,
    },
    Uploader(UploaderState),
    Following(following::FollowingState),
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
        let navigating = self.requested_following
            || self.requested_play.is_some()
            || self.requested_detail.is_some()
            || self.requested_uploader.is_some()
            || self.requested_login
            || self.requested_comments.is_some();
        if navigating || self.go_back {
            self.report_history(true, false);
        }
        if navigating {
            let target = match self.page {
                0 => Some(BackTarget::Home {
                    tab: self.tab,
                    selection: self.sel,
                    first_row: self.first_row,
                    grid_scroll: self.grid_scroll,
                }),
                3 => Some(BackTarget::Detail {
                    bvid: self.detail_bvid.clone(),
                    display: self.detail_display.clone(),
                    from_uploader: self.detail_from_uploader,
                    scroll: self.detail_scroll,
                }),
                7 => Some(BackTarget::Following(std::mem::take(&mut self.following))),
                5 => {
                    self.uploader.selection = self.sel;
                    self.uploader.first_row = self.first_row;
                    self.uploader.grid_scroll = self.grid_scroll;
                    Some(BackTarget::Uploader(std::mem::take(&mut self.uploader)))
                }
                1 => self.current_play.take().map(|info| BackTarget::Play {
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
        if navigating || self.go_back {
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
            self.uploader_request += 1;
            self.following_request += 1;
            self.current_play = None;
            self.danmaku = None;
            self.frames_in_seg = 0;
            self.seek_pending = None;
            self.seek_deadline = None;
            self.left_hold = None;
            self.right_hold = None;
            self.detail_hold = None;
            self.scroll_tap = None;
            self.ui.set_has_frame(false);
            self.ui.set_paused(false);
            self.danmaku_view.clear();
        }
        if self.requested_following {
            self.requested_following = false;
            self.page = 7;
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
            self.page = 2;
            self.ui.set_has_qr(false);
            self.ui.set_qr_status("正在生成二维码…".into());
            self.login_request += 1;
            tasks::start_login(self.login_request, self.tx.clone());
        }
        if let Some(bvid) = self.requested_play.take() {
            self.controls = playback::Controls::default();
            self.page = 1;
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
            self.detail_from_uploader = self.page == 5;
            self.page = 3;
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
        if let Some(owner) = self.requested_uploader.take() {
            self.page = 5;
            self.uploader = UploaderState {
                owner,
                loading: true,
                has_more: true,
                ..Default::default()
            };
            self.sel = 0;
            self.first_row = 0;
            self.grid_scroll = 0.0;
            self.uploader_dirty = true;
            tasks::load_uploader(
                self.uploader.owner.mid.clone(),
                1,
                self.cookies.clone(),
                self.uploader_request,
                self.tx.clone(),
            );
        }
        if self.go_back {
            match self.back_stack.pop() {
                Some(BackTarget::Home {
                    tab: previous_tab,
                    selection,
                    first_row: first,
                    grid_scroll: scroll,
                }) => {
                    self.page = 0;
                    self.tab = previous_tab;
                    self.sel = selection;
                    self.first_row = first;
                    self.grid_scroll = scroll;
                    self.home_dirty = true;
                    self.status.clear();
                    self.status_detail.clear();
                }
                Some(BackTarget::Detail {
                    bvid,
                    display,
                    from_uploader,
                    scroll,
                }) => {
                    self.page = 3;
                    self.detail_bvid = bvid;
                    self.detail_scroll = scroll;
                    self.detail_display = display;
                    self.detail_from_uploader = from_uploader;
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
                Some(BackTarget::Following(previous)) => {
                    self.page = 7;
                    self.following = previous;
                    self.following.loading = false;
                    self.following.dirty = true;
                }
                Some(BackTarget::Uploader(previous)) => {
                    self.page = 5;
                    self.uploader = previous;
                    self.sel = self.uploader.selection;
                    self.first_row = self.uploader.first_row;
                    self.grid_scroll = self.uploader.grid_scroll;
                    self.uploader.loading = false;
                    self.uploader_dirty = true;
                    let missing_covers = self
                        .uploader
                        .items
                        .iter()
                        .zip(&self.uploader.covers)
                        .filter(|(_, cover)| cover.is_none())
                        .map(|(item, _)| item.clone())
                        .collect::<Vec<_>>();
                    if !missing_covers.is_empty() {
                        tasks::spawn_covers(5, missing_covers, self.tx.clone());
                    }
                    if self.uploader.items.is_empty() {
                        self.uploader.loading = true;
                        tasks::load_uploader(
                            self.uploader.owner.mid.clone(),
                            1,
                            self.cookies.clone(),
                            self.uploader_request,
                            self.tx.clone(),
                        );
                    }
                }
                Some(BackTarget::Play {
                    info,
                    position,
                    paused,
                    rate,
                    engine,
                }) => {
                    self.page = 1;
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
                None => self.page = 0,
            }
        }
    }
}
