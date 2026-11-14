use slint::ComponentHandle as _;
use super::*;

impl App {
    pub(super) fn handle_message(&mut self, m: Msg) {
        let g_status = self.ui.global::<crate::Status>();
        let login = self.ui.global::<crate::Login>();
        let detail = self.ui.global::<crate::Detail>();
        match m {
            Msg::Confirmed(accepted) => self.finish_confirmation(accepted),
            Msg::CacheUsage(result) => self.cache_usage(result),
            Msg::CacheCleared(result) => self.cache_cleared(result),
            Msg::CdnSpeed(request, index, result) => self.cdn_result(request, index, result),
            Msg::Chapters(cid, result) => {
                if self.sections.cid == cid {
                    self.sections.loading = false;
                    match result {
                        Ok(chapters) => self.sections.chapters = chapters,
                        Err(error) => self.sections.error = error,
                    }
                    self.sections.dirty = true;
                }
            }
            Msg::Watching(request, Ok(text)) if request == self.play_request => {
                self.ui.global::<crate::Player>().set_watching(text.into())
            }
            Msg::Watching(_, _) => {}
            Msg::Liked(request, result) => {
                if self.page == Page::Player && request == self.play_request {
                    self.controls.busy = false;
                    match result {
                        Ok(liked) => self.controls.liked = Some(liked),
                        Err(error) => self.toast(&error.replace('\n', " · ")),
                    }
                }
            }
            Msg::Reaction(request, result) => {
                if self.page == Page::Player && request == self.play_request {
                    self.controls.busy = false;
                    match result {
                        Ok(result) => {
                            self.finish_triple(result.triple_complete);
                            self.controls.liked = Some(result.liked);
                            self.toast(&result.message);
                        }
                        Err(error) => {
                            self.finish_triple(false);
                            self.toast(&error.replace('\n', " · "));
                        }
                    }
                }
            }
            Msg::Favorites(request, result) => {
                if request != self.favorites.request {
                    return;
                }
                self.favorites.loading = false;
                match result {
                    Ok(folders) => {
                        let selected = self
                            .favorites
                            .folders
                            .get(self.favorites.selection)
                            .map(|folder| folder.id);
                        if let Some(index) = folders
                            .iter()
                            .position(|folder| Some(folder.id) == selected)
                        {
                            self.favorites.selection = index;
                        }
                        let missing = folders
                            .iter()
                            .filter(|folder| !self.favorites.covers.contains_key(&folder.id))
                            .cloned()
                            .collect();
                        tasks::load_favorite_covers(missing, request, self.tx.clone());
                        self.favorites.folders = folders;
                        self.favorites.loaded = true;
                        self.favorites.dirty = true;
                    }
                    Err(error) => self.favorites.error = error,
                }
            }
            Msg::FavoriteCover(request, id, (w, h, pixels)) => {
                if request == self.favorites.request {
                    self.favorites
                        .covers
                        .insert(id, util::raw_to_image(w, h, &pixels));
                    self.favorites.dirty = true;
                }
            }
            Msg::Following(request, page, result) => self.following_loaded(request, page, result),
            Msg::Dynamic(request, offset, result) => {
                if request != self.dynamic_request
                    || (!offset.is_empty() && offset != self.dynamic_offset)
                {
                    return;
                }
                if !offset.is_empty() {
                    self.loading_more[2] = false;
                }
                match result {
                    Ok(page) => {
                        if offset.is_empty() {
                            self.handle_message(Msg::ListLoaded(2, page.items));
                        } else {
                            self.handle_message(Msg::ListMore(2, page.items));
                        }
                        if !offset.is_empty() || self.pages[2] <= 1 && !self.loading_more[2] {
                            self.dynamic_offset = page.offset;
                            self.has_more[2] = page.has_more;
                        }
                        if self.lists[2].is_empty() && page.has_more {
                            self.loading_more[2] = true;
                            tasks::load_dynamic(
                                self.dynamic_request,
                                self.dynamic_offset.clone(),
                                self.cookies.clone(),
                                self.tx.clone(),
                                false,
                            );
                        }
                    }
                    Err(error) => {
                        self.has_more[2] = false;
                        self.handle_message(Msg::Err(2, error));
                    }
                }
            }
            Msg::History(request, first, result) => {
                if request != self.history_request {
                    return;
                }
                if !first {
                    self.loading_more[3] = false;
                }
                match result {
                    Ok(page) => {
                        let update_cursor = !first || self.pages[3] <= 1 && !self.loading_more[3];
                        self.handle_message(if first {
                            Msg::ListLoaded(3, page.items)
                        } else {
                            Msg::ListMore(3, page.items)
                        });
                        if update_cursor {
                            self.history_cursor = page.cursor;
                            self.has_more[3] = page.has_more;
                        }
                    }
                    Err(error) => {
                        self.loading_more[3] = false;
                        self.has_more[3] = false;
                        self.handle_message(Msg::Err(3, error));
                    }
                }
            }
            Msg::Comments(request, page, result) => self.comments_loaded(request, page, result),
            Msg::Volume(result) => {
                if self.page == Page::Player {
                    match result {
                        Ok(volume) => self.toast(&format!("系统音量 {volume}%")),
                        Err(_) => {
                            self.toast("系统音量调整失败");
                        }
                    }
                }
            }
            Msg::Frame(g) => {
                if self.page == Page::Player {
                    if let Some(p) = self.player.as_mut() {
                        if g == p.gen {
                            if let Some((index, pixels)) = p.take_frame() {
                                if !p.paused || self.frames_in_seg == 0 {
                                    p.frame_ready();
                                    self.frames_in_seg = index;
                                    self.latest = Some((g, pixels));
                                }
                            }
                        }
                    }
                }
            }
            Msg::PlaybackEnded(g) => {
                if self.player.as_ref().is_some_and(|p| p.gen == g)
                    && self.current_play.as_ref().is_some_and(|info| {
                        self.player.as_ref().unwrap().position(self.frames_in_seg)
                            >= info.duration - 1.0
                    })
                {
                    self.report_history(true, true);
                }
                if let Some(p) = self.player.as_mut() {
                    if g == p.gen {
                        p.finished = true;
                        crate::diagnostics::write(format_args!(
                            "[播放] 已结束 gen={g} 位置={:.3}s 帧数={}",
                            p.position(self.frames_in_seg),
                            self.frames_in_seg
                        ));
                        self.status = if self.frames_in_seg == 0 {
                            "播放失败，按播放键重试"
                        } else {
                            "播放结束，按 ESC 返回"
                        }
                        .into();
                        self.status_detail.clear();
                    }
                }
            }
            Msg::ListLoaded(t, items) => {
                self.home_dirty = true;
                let selection = refresh_first_page(
                    &mut self.lists[t],
                    &mut self.covers[t],
                    &mut self.first_page_sizes[t],
                    items,
                    self.sel,
                );
                self.loaded[t] = true;
                if self.pages[t] <= 1 {
                    self.has_more[t] = true;
                }
                self.covers_started[t] = true;
                let len = self.lists[t].len();
                let missing = self.lists[t]
                    .iter()
                    .zip(&self.covers[t])
                    .filter(|(_, cover)| cover.is_none())
                    .map(|(item, _)| item.clone())
                    .collect::<Vec<_>>();
                if !missing.is_empty() {
                    tasks::spawn_covers(t, missing, self.tx.clone());
                }
                if list_source(self.tab, self.mine_detail) == Some(t) {
                    self.loading = false;
                }
                if self.page == Page::Home && list_source(self.tab, self.mine_detail) == Some(t) {
                    self.sel = selection;
                    self.status = if len == 0 {
                        "暂无内容".to_string()
                    } else {
                        format!("{} 条", len)
                    };
                    self.status_detail.clear();
                    if let Ok(v) = std::env::var("BILI_SEL") {
                        if let Ok(v) = v.parse::<i32>() {
                            self.sel = v;
                        }
                    }
                }
            }
            Msg::ListMore(t, items) => {
                self.home_dirty = true;
                self.loading_more[t] = false;
                if items.is_empty() {
                    self.has_more[t] = false;
                } else {
                    let mut known: HashSet<String> =
                        self.lists[t].iter().map(|item| item.bvid.clone()).collect();
                    let add: Vec<model::Item> = items
                        .into_iter()
                        .filter(|item| known.insert(item.bvid.clone()))
                        .collect();
                    if add.is_empty() {
                        self.has_more[t] = false;
                    } else {
                        self.pages[t] += 1;
                        self.lists[t].extend(add.clone());
                        self.covers[t].extend(vec![None; add.len()]);
                        if std::env::var("BILI_DUMP").is_ok() {
                            eprintln!(
                                "MORE tab={} page={} add={} total={}",
                                t,
                                self.pages[t],
                                add.len(),
                                self.lists[t].len()
                            );
                        }
                        tasks::spawn_covers(t, add, self.tx.clone());
                        if t == self.tab.index() && self.page == Page::Home {
                            self.status = format!("{} 条", self.lists[t].len());
                            self.status_detail.clear();
                        }
                    }
                }
            }
            Msg::Err(t, msg) => {
                let (friendly, tech) = match msg.split_once('\n') {
                    Some((a, b)) => (a.to_string(), b.to_string()),
                    None => (msg, String::new()),
                };
                if t == 9 {
                    self.status = friendly;
                    self.status_detail = tech;
                } else if list_source(self.tab, self.mine_detail) == Some(t) {
                    self.loading = false;
                    if self.page == Page::Home {
                        self.status = friendly;
                        self.status_detail = tech;
                        g_status.set_bar("加载失败".into());
                        g_status.set_bar_sticky(false);
                    }
                }
            }
            Msg::PlayReady(request, result) => {
                if self.page != Page::Player || request != self.play_request {
                    return;
                }
                match result {
                    Ok(info) => {
                        self.gen += 1;
                        self.frames_in_seg = 0;
                        self.player = Some(player::Player::start(
                            &info,
                            self.cookies.clone().unwrap_or_default(),
                            self.gen,
                            self.tx.clone(),
                            player::PlaybackState {
                                position: std::mem::take(&mut self.play_start),
                                ..Default::default()
                            },
                        ));
                        tasks::start_danmaku(
                            info.cid,
                            self.cookies.clone(),
                            self.gen,
                            self.tx.clone(),
                        );
                        self.setup_sections(info.bvid.clone(), info.cid, info.parts.clone());
                        tasks::playback::watching(
                            info.bvid.clone(),
                            info.cid,
                            self.cookies.clone(),
                            self.play_request,
                            self.tx.clone(),
                        );
                        self.current_play = Some(info);
                        self.refresh_liked();
                        self.status = "加载中…".into();
                        self.status_detail.clear();
                    }
                    Err(error) => {
                        self.play_start = 0.0;
                        self.status = error;
                        self.status_detail.clear();
                    }
                }
            }
            Msg::Danmaku(g, eng) => {
                if g == self.gen {
                    self.danmaku_view.clear();
                    let t = self
                        .player
                        .as_ref()
                        .map(|p| p.position(self.frames_in_seg))
                        .unwrap_or(0.0);
                    if self.settings.danmaku_enabled {
                        self.danmaku_view.update(&eng, t);
                    }
                    self.danmaku = Some(eng);
                }
            }
            Msg::QrReady(request, url) => {
                if request != self.login_request || self.page != Page::Login {
                    return;
                }
                if let Some(img) = util::make_qr(&url) {
                    login.set_qr(img);
                    login.set_has_qr(true);
                }
                login.set_qr_status("请用 B 站 App 扫码".into());
            }
            Msg::QrStatus(request, s) => {
                if request != self.login_request || self.page != Page::Login {
                    return;
                }
                login.set_qr_status(s.as_str().into());
            }
            Msg::QrDone(request, ck) => {
                if request != self.login_request || self.page != Page::Login {
                    return;
                }
                if !ck.is_empty() {
                    if let Some(dir) = std::path::Path::new(&self.cpath).parent() {
                        if let Err(error) = std::fs::create_dir_all(dir) {
                            crate::diagnostics::write(format_args!(
                                "[登录] 创建登录态目录失败: {error}"
                            ));
                        }
                    }
                    let saved = std::fs::write(&self.cpath, &ck);
                    crate::diagnostics::write(format_args!(
                        "[登录] 登录成功 保存登录态结果={saved:?}"
                    ));
                    self.mid = util::parse_mid(&ck);
                    self.cookies = Some(ck);
                    self.account_dirty = true;
                    login.set_qr_status("登录成功".into());
                    self.acct_loaded = false;
                    self.acct = None;
                    self.avatar_img = None;
                    self.favorites = favorites::Favorites {
                        request: self.favorites.request + 1,
                        ..Default::default()
                    };
                    if self.login_favorites {
                        self.login_favorites = false;
                        self.requested_favorites = true;
                        return;
                    }
                    if self.login_following {
                        self.login_following = false;
                        self.requested_following = true;
                        return;
                    }
                    if let Some(owner) = self.login_uploader.take() {
                        self.login_return = false;
                        self.requested_uploader = Some(owner);
                        return;
                    }
                    if self.login_return {
                        self.login_return = false;
                        self.go_back = true;
                        return;
                    }
                    self.go_back = true;
                }
            }
            Msg::Acct(account, avatar) => {
                self.account_dirty = true;
                self.avatar_img = avatar.map(|(w, h, raw)| util::raw_to_image(w, h, &raw));
                self.acct = Some(account);
            }
            Msg::FollowingAvatar(request, mid, (w, h, raw)) => {
                if request == self.following_request {
                    self.following
                        .avatars
                        .insert(mid, util::raw_to_image(w, h, &raw));
                    self.following.dirty = true;
                }
            }
            Msg::Cover(t, bvid, w, h, raw) => {
                if t < 5 {
                    if let Some(idx) = self.lists[t].iter().position(|it| it.bvid == bvid) {
                        if idx < self.covers[t].len() {
                            self.covers[t][idx] = Some(util::raw_to_image(w, h, &raw));
                            self.home_dirty = true;
                        }
                    }
                } else if t == 5 {
                    if let Some(index) = self
                        .collection
                        .items
                        .iter()
                        .position(|item| item.bvid == bvid)
                    {
                        if let Some(cover) = self.collection.covers.get_mut(index) {
                            *cover = Some(util::raw_to_image(w, h, &raw));
                            self.collection_dirty = true;
                        }
                    }
                }
            }
            Msg::Detail(request, data, w, h, raw) => {
                if self.page != Page::Detail || request != self.detail_request {
                    return;
                }
                let display = DetailDisplay {
                    data,
                    cover: if raw.is_empty() {
                        None
                    } else {
                        Some(util::raw_to_image(w, h, &raw))
                    },
                };
                if self.comment_page.aid != display.data.aid {
                    self.start_comments(display.data.aid);
                }
                self.setup_sections(
                    self.detail_bvid.clone(),
                    display.data.cid,
                    display.data.parts.clone(),
                );
                self.marq_t = 0.0;
                show_detail(&self.ui, &display);
                self.detail_display = Some(display);
            }
            Msg::DetailError(request, error) => {
                if self.page == Page::Detail && request == self.detail_request {
                    detail.set_loading(false);
                    detail.set_error(error.into());
                }
            }
            Msg::AuthorProfile(request, result) => {
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
            Msg::Collection(request, loaded_page, result) => {
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
        }
    }
}
