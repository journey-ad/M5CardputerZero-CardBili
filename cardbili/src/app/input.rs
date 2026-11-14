use slint::ComponentHandle as _;
use super::*;

pub(super) struct DirectionHold {
    key: &'static str,
    since: Instant,
    repeat_at: Instant,
}

impl App {
    pub(super) fn handle_key(&mut self, mut key: String) {
        if self.confirmation.is_some() {
            self.confirmation_key(&key);
            return;
        }
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
            if self.page == Page::Player && !self.sections.open && !self.ui.global::<crate::Player>().get_controls_visible() {
                self.last_input = Instant::now();
                return;
            }
            let Some(action) = self.menu_actions()[number - 4] else {
                return;
            };
            if action == menu::Action::Back {
                self.direction_hold = None;
                self.return_hold = Some((number, Instant::now()));
                return;
            }
            key = action.key().to_string();
        }
        if let Some(released) = key.strip_suffix("_UP") {
            if self
                .direction_hold
                .as_ref()
                .is_some_and(|hold| hold.key == released)
            {
                let hold = self.direction_hold.take().unwrap();
                if matches!(self.page, Page::Detail | Page::Comments)
                    && matches!(released, "UP" | "DOWN")
                    && hold.since.elapsed() <= Duration::from_millis(300)
                {
                    let step = if released == "UP" { -22.0 } else { 22.0 };
                    self.scroll_tap = Some((step, hold.since));
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
                since: now,
                repeat_at: now + Duration::from_millis(350),
            });
            self.dispatch_key(key, false);
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
            self.last_input = now;
            self.dispatch_key(key, true);
        }
    }

    fn dispatch_key(&mut self, key: &str, repeat: bool) {
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
            Page::Detail => self.detail_key(key, repeat),
            Page::About => self.about_key(key),
            Page::Collection => self.collection_key(key),
            Page::Comments => self.comments_key(key, repeat),
            Page::Following => self.following_key(key),
            Page::Favorites => self.favorites_key(key),
        }
    }
}
