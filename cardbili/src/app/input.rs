use slint::ComponentHandle as _;
use super::*;

pub(super) struct DirectionHold {
    key: &'static str,
    repeat_at: Instant,
    pub(super) repeat_started: Option<Instant>,
}

impl App {
    // 播放页按键帽上印的媒体图标处理：Q 暂停、W/E 快退/快进、A 静音、S/D 音量减/加
    // 其他页面 W 仍为上；返回 None 表示丢弃该按键
    pub(super) fn keycap_key<'a>(&self, key: &'a str, super_chat_open: bool) -> Option<&'a str> {
        let player = self.ui.global::<crate::Player>();
        let media = self.page == Page::Player
            && !super_chat_open
            && !self.sections.open
            && !player.get_menu_open();
        if !media {
            return match key {
                "W_UP" => Some("UP_UP"),
                "E_UP" | "D_UP" => None,
                _ => Some(key),
            };
        }
        let live = player.get_live();
        Some(match key {
            "q" => "SPACE",
            "a" => "MUTE",
            "d" => "UP",
            "D_UP" => "UP_UP",
            "w" | "e" | "W_UP" | "E_UP" if live => return None,
            "w" => "LEFT",
            "e" => "RIGHT",
            "W_UP" => "LEFT_UP",
            "E_UP" => "RIGHT_UP",
            _ => key,
        })
    }

    pub(super) fn handle_key(&mut self, mut key: String) {
        if self.composer.target.is_some() {
            self.composer_key(&key);
            return;
        }
        if self.ui.global::<crate::SettingsChoice>().get_open() {
            self.settings_choice_key(&key);
            return;
        }
        if matches!(key.as_str(), "7_UP" | "DANMAKU_UP" | "DANMAKU_CANCEL") {
            if self.danmaku_hold.take().is_some() && key != "DANMAKU_CANCEL" {
                self.toggle_danmaku();
            }
            return;
        }
        if self.confirmation.is_some() {
            self.confirmation_key(&key);
            return;
        }
        // Esc 按满 2 秒只发送 ESC_HOLD：首页视频页签用于退出；其他页面与长按底栏返回一致，回到首页
        if key == "ESC_HOLD" {
            if self.page != Page::Home {
                self.direction_hold = None;
                self.return_home = true;
                return;
            }
            if !self.tab.is_video() || self.help {
                key = "ESC".into();
            }
        }
        let super_chat_open = self.ui.global::<crate::SuperChat>().get_open();
        let Some(mapped) = self.keycap_key(&key, super_chat_open) else {
            return;
        };
        key = mapped.to_string();
        if let Some(button) = key
            .strip_suffix("_UP")
            .and_then(|s| s.parse::<usize>().ok())
        {
            if self.return_hold.is_some_and(|(held, _)| held == button) {
                self.return_hold = None;
                let now = Instant::now();
                let consecutive = self
                    .back_taps
                    .1
                    .is_some_and(|last| now.duration_since(last) < Duration::from_secs(1));
                self.back_taps = (
                    if consecutive {
                        self.back_taps.0.saturating_add(1)
                    } else {
                        1
                    },
                    Some(now),
                );
                if self.back_taps.0 > 3 || self.back_hint {
                    self.back_hint = true;
                    self.bar_until = Some(now + Duration::from_secs(3));
                }
                self.dispatch_key("ESC", false);
            }
            return;
        }
        if let Ok(number @ 4..=8) = key.parse::<usize>() {
            if super_chat_open {
                self.super_chat_key(&key);
                return;
            }
            let Some(action) = self.menu_actions()[number - 4] else {
                return;
            };
            if action != menu::Action::More
                && self.page == Page::Player
                && !self.sections.open
                && !self.ui.global::<crate::Player>().get_controls_visible()
            {
                self.last_input = Instant::now();
                return;
            }
            if action == menu::Action::Back {
                self.direction_hold = None;
                self.return_hold = Some((number, Instant::now()));
                return;
            }
            key = action.key().to_string();
        }
        if key == "DANMAKU" && self.page == Page::Player {
            self.danmaku_hold.get_or_insert_with(Instant::now);
            self.last_input = Instant::now();
            return;
        }
        if let Some(released) = key.strip_suffix("_UP") {
            if self
                .direction_hold
                .as_ref()
                .is_some_and(|hold| hold.key == released)
            {
                let hold = self.direction_hold.take().unwrap();
                if self.page == Page::Player
                    && !super_chat_open
                    && matches!(released, "LEFT" | "RIGHT")
                {
                    if hold.repeat_started.is_none() {
                        self.dispatch_key(released, false);
                    }
                    if self.seek_pending.is_some() {
                        self.seek_deadline = Some(Instant::now());
                    }
                }
                return;
            }
        }
        self.last_input = Instant::now();
        if !key.ends_with("_UP") && key != "POINTER" {
            self.back_taps = (0, None);
            self.back_hint = false;
        }
        let direction = match key.as_str() {
            "UP" | "f" | "w" => Some("UP"),
            "DOWN" | "x" | "s" => Some("DOWN"),
            "LEFT" | "z" => Some("LEFT"),
            "RIGHT" | "c" => Some("RIGHT"),
            _ => None,
        };
        if let Some(key) = direction {
            let now = Instant::now();
            self.direction_hold = Some(DirectionHold {
                key,
                repeat_at: now + Duration::from_millis(350),
                repeat_started: None,
            });
            // 播放页左右键长按用于快进快退，按下先不跳转，松开后再提交
            if super_chat_open || !(self.page == Page::Player && matches!(key, "LEFT" | "RIGHT")) {
                self.dispatch_key(key, false);
            }
        } else {
            if !key.ends_with("_UP") && key != "POINTER" {
                self.direction_hold = None;
            }
            self.dispatch_key(&key, false);
        }
    }

    pub(super) fn tick_input(&mut self) {
        let now = Instant::now();
        if self
            .danmaku_hold
            .is_some_and(|since| since.elapsed() >= Duration::from_millis(800))
        {
            self.danmaku_hold = None;
            self.open_danmaku_input();
        }
        if self
            .return_hold
            .is_some_and(|(_, since)| now.duration_since(since) >= Duration::from_millis(800))
        {
            self.return_hold = None;
            self.return_home = true;
            self.back_hint = false;
            self.back_taps = (0, None);
        }
        let repeat = self.direction_hold.as_mut().and_then(|hold| {
            if now < hold.repeat_at {
                return None;
            }
            hold.repeat_at = now + Duration::from_millis(120);
            Some(hold.key)
        });
        if let Some(key) = repeat {
            if let Some(hold) = self.direction_hold.as_mut() {
                hold.repeat_started.get_or_insert(now);
            }
            self.last_input = now;
            self.dispatch_key(key, true);
        }
    }

    fn dispatch_key(&mut self, key: &str, repeat: bool) {
        if self.ui.global::<crate::SuperChat>().get_open() {
            self.super_chat_key(key);
            return;
        }
        if self.ui.global::<crate::LiveAreas>().get_open() {
            self.live_area_key(key);
            return;
        }
        if self.ui.global::<crate::Cdn>().get_open() {
            self.cdn_key(key);
            return;
        }
        if self.sections.open {
            self.sections_key(key);
            return;
        }
        match self.page {
            Page::Home => self.home_key(key),
            Page::Player => self.play_key(key, repeat),
            Page::Login => self.login_key(key),
            Page::Detail => self.detail_key(key),
            Page::About => self.about_key(key),
            Page::Collection => self.collection_key(key),
            Page::Comments => self.comments_key(key),
            Page::Following => self.following_key(key),
            Page::Favorites => self.favorites_key(key),
        }
    }
}
