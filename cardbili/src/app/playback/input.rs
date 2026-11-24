use super::super::*;
use slint::ComponentHandle as _;
use slint::Model as _;

impl App {
    // 直播支持暂停、重连、弹幕开关、醒目留言、音量与返回
    fn live_key(&mut self, k: &str) {
        if k == "SUPER_CHAT" {
            self.open_super_chat();
            return;
        }
        if k == "r" {
            self.requested_live = self.live.as_ref().map(|info| info.room_id);
            return;
        }
        if k == "DANMAKU" {
            self.toggle_danmaku();
            return;
        }
        if let Some(p) = self.player.as_mut() {
            match k {
                "SPACE" | "ENTER" => {
                    if p.finished || p.paused {
                        self.requested_live = self.live.as_ref().map(|info| info.room_id);
                    } else {
                        p.pause_toggle();
                    }
                }
                "UP" | "f" => {
                    let _ = self.volume_queue.try_send(player::volume::Change::Step(10));
                }
                "DOWN" | "x" | "s" => {
                    let _ = self.volume_queue.try_send(player::volume::Change::Step(-10));
                }
                "MUTE" => {
                    let _ = self.volume_queue.try_send(player::volume::Change::ToggleMute);
                }
                "ESC" | "BACKSPACE" => self.go_back = true,
                _ => {}
            }
        } else if matches!(k, "SPACE" | "ENTER") {
            self.requested_live = self.live.as_ref().map(|info| info.room_id);
        } else if matches!(k, "ESC" | "BACKSPACE") {
            self.go_back = true;
        }
    }

    pub(in crate::app) fn play_key(&mut self, k: &str, repeat: bool) {
        // 统计浮层打开时，返回键只关闭浮层
        if self.ui.global::<crate::Player>().get_stats_on()
            && matches!(k, "ESC" | "BACKSPACE" | "ENTER" | "SPACE")
        {
            self.ui.global::<crate::Player>().set_stats_on(false);
            return;
        }
        if let Some(action) = k.strip_prefix("menu:") {
            self.menu_action(action);
            return;
        }
        if let Some(raw) = k.strip_prefix("audio:") {
            if let Ok(label) = raw.parse::<i32>() {
                self.select_audio(label);
                self.ui.global::<crate::Player>().set_menu_open(false);
            }
            return;
        }
        if let Some(raw) = k.strip_prefix("sub:") {
            if let Ok(index) = raw.parse::<i32>() {
                self.apply_subtitle(index);
                self.ui.global::<crate::Player>().set_menu_open(false);
            }
            return;
        }
        if let Some(raw) = k.strip_prefix("fav:") {
            if let Ok(index) = raw.parse::<usize>() {
                if let Some(folder) = self.favorites.folders.get(index) {
                    self.react(model::VideoAction::Favorite(folder.id));
                    self.ui.global::<crate::Player>().set_menu_open(false);
                }
            }
            return;
        }
        // 更多菜单打开时，方向键与回车在菜单内导航
        if self.ui.global::<crate::Player>().get_menu_open() {
            self.menu_key(k);
            return;
        }
        if k == "m" {
            self.toggle_menu();
            return;
        }
        if self.ui.global::<crate::Player>().get_live() {
            return self.live_key(k);
        }
        if k == "p" {
            self.sections.open = true;
            self.sections.dirty = true;
            return;
        }
        match k {
            "l" => {
                self.like_press();
                return;
            }
            "LIKE_UP" => {
                self.like_release(false);
                return;
            }
            "LIKE_CANCEL" => {
                self.like_release(true);
                return;
            }
            "b" => {
                self.change_speed();
                return;
            }
            "SUBTITLE" => {
                self.toggle_subtitle();
                return;
            }
            "DANMAKU" => {
                self.toggle_danmaku();
                return;
            }
            "r" => {
                self.requested_comments = self
                    .current_play
                    .as_ref()
                    .map(|info| info.aid)
                    .filter(|aid| *aid > 0);
                return;
            }
            _ => {}
        }
        if matches!(k, "SPACE" | "ENTER" | "LEFT" | "z" | "RIGHT" | "c") {
            self.report_history(true, false);
        }
        let g_player = self.ui.global::<crate::Player>();
        if let Some(p) = self.player.as_mut() {
            match k {
                "SPACE" | "ENTER" => {
                    if p.finished {
                        // 重试重新获取当前分 P 的播放地址
                        if let Some(info) = &self.current_play {
                            self.play_request += 1;
                            self.seek_pending = None;
                            self.seek_deadline = None;
                            self.latest = None;
                            g_player.set_has_frame(false);
                            self.status = crate::i18n::tr("加载中…").into();
                            self.status_detail.clear();
                            tasks::start_play(
                                info.bvid.clone(),
                                Some(info.cid),
                                self.cookies.clone(),
                                self.play_request,
                                self.settings.cdn.clone(),
                                self.tx.clone(),
                                true,
                            );
                        }
                    } else {
                        p.pause_toggle();
                    }
                }
                "LEFT" | "RIGHT" => {
                    let step = if repeat {
                        let elapsed = self
                            .direction_hold
                            .as_ref()
                            .and_then(|hold| hold.repeat_started)
                            .map_or(0.0, |started| started.elapsed().as_secs_f64());
                        (2.0 + elapsed * 7.0).min(30.0)
                    } else {
                        10.0
                    };
                    let step = if k == "LEFT" { -step } else { step };
                    let base = self
                        .seek_pending
                        .unwrap_or_else(|| p.position(self.frames_in_seg));
                    let position = (base + step).clamp(0.0, p.duration.max(0.0));
                    g_player.set_progress(if p.duration > 0.0 {
                        (position / p.duration) as f32
                    } else {
                        0.0
                    });
                    g_player.set_time_text(
                        format!("{} / {}", model::mmss(position), model::mmss(p.duration)).into(),
                    );
                    self.seek_pending = Some(position);
                    self.seek_deadline = Some(Instant::now() + Duration::from_millis(300));
                }
                "UP" | "f" => {
                    let _ = self.volume_queue.try_send(player::volume::Change::Step(10));
                }
                "DOWN" | "x" | "s" => {
                    let _ = self.volume_queue.try_send(player::volume::Change::Step(-10));
                }
                "MUTE" => {
                    let _ = self.volume_queue.try_send(player::volume::Change::ToggleMute);
                }
                "u" => {
                    self.requested_uploader = self
                        .current_play
                        .as_ref()
                        .map(|info| info.owner.clone())
                        .filter(|owner| !owner.mid.is_empty());
                }
                "ESC" | "BACKSPACE" => {
                    self.go_back = true;
                }
                _ => {}
            }
        } else if matches!(k, "ESC" | "BACKSPACE") {
            self.go_back = true;
        }
    }
}

// 更多菜单与技术统计、字幕、音轨操作
impl App {
    fn toggle_menu(&mut self) {
        let g_player = self.ui.global::<crate::Player>();
        g_player.set_menu_index(0);
        g_player.set_track_menu(crate::TrackMenu::Closed);
        g_player.set_menu_open(!g_player.get_menu_open());
    }

    // 菜单导航：上下移动，回车执行，esc 或 m 关闭，选项行回车展开下拉
    fn menu_key(&mut self, k: &str) {
        let g_player = self.ui.global::<crate::Player>();
        if g_player.get_track_menu() != crate::TrackMenu::Closed {
            let kind = g_player.get_track_menu();
            let len = g_player.get_track_labels().row_count() as i32;
            if len == 0 {
                if matches!(k, "ESC" | "BACKSPACE" | "LEFT") {
                    g_player.set_track_menu(crate::TrackMenu::Closed);
                } else if k == "m" {
                    g_player.set_menu_open(false);
                }
                return;
            }
            match k {
                "UP" | "k" => {
                    g_player.set_menu_sub_index((g_player.get_menu_sub_index() - 1 + len) % len)
                }
                "DOWN" | "j" => {
                    g_player.set_menu_sub_index((g_player.get_menu_sub_index() + 1) % len)
                }
                "LEFT" => g_player.set_track_menu(crate::TrackMenu::Closed),
                "ENTER" | "SPACE" => {
                    let index = g_player.get_menu_sub_index();
                    if kind == crate::TrackMenu::Subtitle {
                        self.apply_subtitle(index);
                    } else if kind == crate::TrackMenu::Audio {
                        self.select_audio(index);
                    } else if let Some(folder) = self.favorites.folders.get(index as usize) {
                        self.react(model::VideoAction::Favorite(folder.id));
                    }
                    self.ui.global::<crate::Player>().set_menu_open(false);
                    return;
                }
                "ESC" | "BACKSPACE" => g_player.set_track_menu(crate::TrackMenu::Closed),
                "m" => g_player.set_menu_open(false),
                _ => {}
            }
            let player = self.ui.global::<crate::Player>();
            player.set_menu_sub_first(ensure_visible(
                player.get_menu_sub_first() as usize,
                player.get_menu_sub_index() as usize,
                player.get_track_visible_count() as usize,
            ) as i32);
            return;
        }
        let items = g_player.get_menu_items();
        let len = items.row_count() as i32;
        match k {
            "UP" | "k" => g_player.set_menu_index((g_player.get_menu_index() - 1 + len) % len),
            "DOWN" | "j" => g_player.set_menu_index((g_player.get_menu_index() + 1) % len),
            "ENTER" | "SPACE" => {
                let item = items.row_data(g_player.get_menu_index() as usize).unwrap();
                self.menu_action(&item.action);
            }
            "ESC" | "BACKSPACE" | "m" => g_player.set_menu_open(false),
            _ => {}
        }
    }

    fn menu_action(&mut self, action: &str) {
        match action {
            "speed" => self.change_speed(),
            "coin" | "favorite" => {
                if self.cookies.is_none() {
                    self.ui.global::<crate::Player>().set_menu_open(false);
                    self.requested_login = true;
                    return;
                }
                if action == "coin" {
                    self.ui.global::<crate::Player>().set_menu_open(false);
                    self.confirm(super::super::confirmation::Action::Coin,
                        crate::i18n::tr("投币"), crate::i18n::tr("向此视频投 1 枚硬币？"), crate::i18n::tr("投币"));
                } else {
                    let player = self.ui.global::<crate::Player>();
                    player.set_track_menu(crate::TrackMenu::Favorite);
                    player.set_menu_sub_index(0);
                    player.set_menu_sub_first(0);
                    if !self.favorites.loaded {
                        self.load_favorites(false);
                    }
                    self.show_favorite_choices();
                }
            }
            "comments" => {
                self.ui.global::<crate::Player>().set_menu_open(false);
                self.play_key("r", false);
            }
            "subtitle" => {
                let g_player = self.ui.global::<crate::Player>();
                g_player.set_track_menu(crate::TrackMenu::Subtitle);
                g_player.set_menu_sub_index(g_player.get_subtitle_selected());
                g_player.set_menu_sub_first((g_player.get_subtitle_selected() - 5).max(0));
            }
            "audio" => {
                let g_player = self.ui.global::<crate::Player>();
                g_player.set_track_menu(crate::TrackMenu::Audio);
                g_player.set_menu_sub_index(g_player.get_audio_selected());
                g_player.set_menu_sub_first((g_player.get_audio_selected() - 5).max(0));
            }
            "stats" => {
                let g_player = self.ui.global::<crate::Player>();
                g_player.set_stats_on(!g_player.get_stats_on());
                g_player.set_menu_open(false);
            }
            "close" => self.ui.global::<crate::Player>().set_menu_open(false),
            _ => {}
        }
    }
}
