use super::*;

impl App {
    pub(super) fn handle_message(&mut self, m: Msg) {
        match m {
            Msg::Liked(request, result) => {
                if self.page == 1 && request == self.play_request {
                    self.controls.busy = false;
                    match result {
                        Ok(liked) => self.controls.liked = Some(liked),
                        Err(error) => self.toast(&error.replace('\n', " · ")),
                    }
                }
            }
            Msg::Reaction(request, result) => {
                if self.page == 1 && request == self.play_request {
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
            Msg::Following(request, page, result) => self.following_loaded(request, page, result),
            Msg::Dynamic(request, offset, result) => {
                if request != self.dynamic_request || offset != self.dynamic_offset {
                    return;
                }
                self.loading_more[2] = false;
                match result {
                    Ok(page) => {
                        if offset.is_empty() {
                            self.handle_message(Msg::ListLoaded(2, page.items));
                        } else {
                            self.handle_message(Msg::ListMore(2, page.items));
                        }
                        self.dynamic_offset = page.offset;
                        self.has_more[2] = page.has_more;
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
            Msg::Comments(request, page, result) => self.comments_loaded(request, page, result),
            Msg::Volume(result) => {
                if self.page == 1 {
                    match result {
                        Ok(volume) => self.toast(&format!("系统音量 {volume}%")),
                        Err(error) => {
                            eprintln!("系统音量调整失败: {error}");
                            self.toast("系统音量调整失败");
                        }
                    }
                }
            }
            Msg::Frame(g) => {
                if self.page == 1 {
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
            Msg::SegEnd(g) => {
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
                        if p.advance() {
                            self.frames_in_seg = 0;
                        } else {
                            self.status = "播放结束，按 ESC 返回".into();
                            self.status_detail.clear();
                        }
                    }
                }
            }
            Msg::ListLoaded(t, items) => {
                self.home_dirty = true;
                let first = !self.loaded[t];
                // 内容与已展示的一致时保留封面与选中，避免 SWR 更新时闪烁
                let same_ids = !first
                    && self.lists[t].len() == items.len()
                    && self.lists[t]
                        .iter()
                        .zip(items.iter())
                        .all(|(a, b)| a.bvid == b.bvid);
                self.loaded[t] = true;
                self.pages[t] = 1;
                self.has_more[t] = true;
                self.loading_more[t] = false;
                let len = items.len();
                if t < 5 {
                    if same_ids {
                        self.covers[t].resize_with(len, || None);
                    } else {
                        self.covers[t] = vec![None; len];
                    }
                    self.covers_started[t] = true;
                    let missing = items
                        .iter()
                        .zip(&self.covers[t])
                        .filter(|(_, cover)| cover.is_none())
                        .map(|(item, _)| item.clone())
                        .collect::<Vec<_>>();
                    if !missing.is_empty() {
                        tasks::spawn_covers(t, missing, self.tx.clone());
                    }
                }
                self.lists[t] = items;
                if list_source(self.tab, self.mine_detail) == Some(t) {
                    self.loading = false;
                }
                if self.page == 0 && list_source(self.tab, self.mine_detail) == Some(t) {
                    if first {
                        self.sel = 0;
                    } else if self.sel >= len as i32 {
                        self.sel = if len == 0 { 0 } else { len as i32 - 1 };
                    }
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
                        if t == self.tab && self.page == 0 {
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
                    if self.page == 0 {
                        self.status = friendly;
                        self.status_detail = tech;
                        self.ui.set_bar("加载失败".into());
                        self.ui.set_bar_sticky(false);
                    }
                }
            }
            Msg::PlayReady(request, result) => {
                if self.page != 1 || request != self.play_request {
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
                            player::PlaybackState::default(),
                        ));
                        tasks::start_danmaku(
                            info.cid,
                            self.cookies.clone(),
                            self.gen,
                            self.tx.clone(),
                        );
                        self.current_play = Some(info);
                        self.refresh_liked();
                        self.status = "加载中…".into();
                        self.status_detail.clear();
                    }
                    Err(error) => {
                        self.status = error;
                        self.status_detail.clear();
                    }
                }
            }
            Msg::Danmaku(g, list) => {
                if g == self.gen {
                    let eng = danmaku::Engine::new(list);
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
                if request != self.login_request || self.page != 2 {
                    return;
                }
                if let Some(img) = util::make_qr(&url) {
                    self.ui.set_qr(img);
                    self.ui.set_has_qr(true);
                }
                self.ui.set_qr_status("请用 B 站 App 扫码".into());
            }
            Msg::QrStatus(request, s) => {
                if request != self.login_request || self.page != 2 {
                    return;
                }
                self.ui.set_qr_status(s.as_str().into());
            }
            Msg::QrDone(request, ck) => {
                if request != self.login_request || self.page != 2 {
                    return;
                }
                if !ck.is_empty() {
                    if let Some(dir) = std::path::Path::new(&self.cpath).parent() {
                        let _ = std::fs::create_dir_all(dir);
                    }
                    let _ = std::fs::write(&self.cpath, &ck);
                    self.mid = util::parse_mid(&ck);
                    self.cookies = Some(ck);
                    self.account_dirty = true;
                    self.ui.set_qr_status("登录成功".into());
                    self.acct_loaded = false;
                    self.acct = None;
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
                    self.page = 0;
                    if self.tab >= 3 {
                        self.loading = false;
                        self.status = "已登录".into();
                        self.status_detail.clear();
                    } else {
                        self.loaded[self.tab] = false;
                        self.loading = true;
                        self.status = "已登录，刷新中…".into();
                        self.status_detail.clear();
                        self.load_home_list(self.tab, false);
                    }
                }
            }
            Msg::Acct(a, raw) => {
                self.account_dirty = true;
                if !raw.is_empty() {
                    if let Some(img) = util::decode_avatar(&raw) {
                        self.avatar_img = Some(img);
                    }
                }
                self.acct = Some(a);
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
                        .uploader
                        .items
                        .iter()
                        .position(|item| item.bvid == bvid)
                    {
                        if let Some(cover) = self.uploader.covers.get_mut(index) {
                            *cover = Some(util::raw_to_image(w, h, &raw));
                            self.uploader_dirty = true;
                        }
                    }
                }
            }
            Msg::Detail(request, data, w, h, raw) => {
                if self.page != 3 || request != self.detail_request {
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
                self.marq_t = 0.0;
                show_detail(&self.ui, &display);
                self.detail_display = Some(display);
            }
            Msg::DetailError(request, error) => {
                if self.page == 3 && request == self.detail_request {
                    self.ui.set_detail_loading(false);
                    self.ui.set_detail_error(error.into());
                }
            }
            Msg::Uploader(request, loaded_page, result) => {
                if self.page != 5 || request != self.uploader_request {
                    return;
                }
                self.uploader.loading = false;
                match result {
                    Ok(videos) => {
                        if loaded_page == 1 {
                            self.uploader.items.clear();
                            self.uploader.covers.clear();
                        }
                        let mut known: HashSet<String> = self
                            .uploader
                            .items
                            .iter()
                            .map(|item| item.bvid.clone())
                            .collect();
                        let previous_len = self.uploader.items.len();
                        self.uploader.items.extend(
                            videos
                                .items
                                .into_iter()
                                .filter(|item| known.insert(item.bvid.clone())),
                        );
                        self.uploader
                            .covers
                            .resize_with(self.uploader.items.len(), || None);
                        tasks::spawn_covers(
                            5,
                            self.uploader.items[previous_len..].to_vec(),
                            self.tx.clone(),
                        );
                        self.uploader.page = loaded_page;
                        self.uploader.has_more =
                            videos.has_more && self.uploader.items.len() > previous_len;
                        self.uploader.error.clear();
                    }
                    Err(error) => {
                        self.uploader.error = error;
                    }
                }
                self.uploader_dirty = true;
            }
            Msg::Stats(bvid, result) => {
                if let Ok(stats) = result {
                    self.stats_cache.insert(bvid, stats);
                    self.home_dirty = true;
                    self.uploader_dirty = true;
                }
            }
        }
    }
}
