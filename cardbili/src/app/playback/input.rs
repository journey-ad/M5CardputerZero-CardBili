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
        if k == "d" {
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
                "UP" | "f" | "w" => {
                    let _ = self.volume_queue.try_send(10);
                }
                "DOWN" | "x" | "s" => {
                    let _ = self.volume_queue.try_send(-10);
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
        // 更多菜单打开时，方向键与回车在菜单内导航
        if self.ui.global::<crate::Player>().get_menu_open() {
            self.menu_key(k);
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
        if k == "m" {
            self.toggle_menu();
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
            "d" => {
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
                        // 播放结束后再次触发播放，从头开始
                        p.paused = false;
                        self.seek_pending = Some(0.0);
                        self.seek_deadline = Some(Instant::now());
                        crate::diagnostics::write(format_args!("[播放] 重播 gen={}", p.gen));
                    } else {
                        p.pause_toggle();
                    }
                }
                "LEFT" | "RIGHT" => {
                    let step = if repeat { 2.0 } else { 10.0 };
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
                "UP" | "f" | "w" => {
                    let _ = self.volume_queue.try_send(10);
                }
                "DOWN" | "x" | "s" => {
                    let _ = self.volume_queue.try_send(-10);
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
                g_player.set_track_menu(crate::TrackMenu::Closed);
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
                    } else {
                        self.select_audio(index);
                    }
                    self.ui.global::<crate::Player>().set_menu_open(false);
                    return;
                }
                "ESC" | "BACKSPACE" => g_player.set_track_menu(crate::TrackMenu::Closed),
                "m" => g_player.set_menu_open(false),
                _ => {}
            }
            return;
        }
        match k {
            "UP" | "k" => g_player.set_menu_index((g_player.get_menu_index() - 1).max(0)),
            "DOWN" | "j" => g_player.set_menu_index((g_player.get_menu_index() + 1).min(3)),
            "ENTER" | "SPACE" => match g_player.get_menu_index() {
                0 => self.menu_action("comments"),
                1 => self.menu_action("subtitle"),
                2 => self.menu_action("audio"),
                _ => self.menu_action("stats"),
            },
            "ESC" | "BACKSPACE" | "m" => g_player.set_menu_open(false),
            _ => {}
        }
    }

    fn menu_action(&mut self, action: &str) {
        match action {
            "comments" => {
                self.ui.global::<crate::Player>().set_menu_open(false);
                self.play_key("r", false);
            }
            "subtitle" => {
                let g_player = self.ui.global::<crate::Player>();
                g_player.set_track_menu(crate::TrackMenu::Subtitle);
                g_player.set_menu_sub_index(g_player.get_subtitle_selected());
            }
            "audio" => {
                let g_player = self.ui.global::<crate::Player>();
                g_player.set_track_menu(crate::TrackMenu::Audio);
                g_player.set_menu_sub_index(g_player.get_audio_selected());
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
