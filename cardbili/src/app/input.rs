use super::*;

pub(super) struct DirectionHold {
    key: &'static str,
    since: Instant,
    repeat_at: Instant,
}

impl App {
    pub(super) fn handle_key(&mut self, mut key: String) {
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
                    if consecutive { self.back_taps.0 + 1 } else { 1 },
                    Some(now),
                );
                if self.back_taps.0 > 3 {
                    self.back_hint = true;
                    self.back_taps.0 = 0;
                }
                self.dispatch_key("ESC", false);
            }
            return;
        }
        if let Ok(number @ 4..=8) = key.parse::<usize>() {
            if self.page == 1 && !self.ui.get_controls_visible() {
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
                if matches!(self.page, 3 | 6)
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
        if matches!(self.page, 3 | 6) && self.scroll_key(key, repeat) {
            return;
        }
        let video_len = if self.page == 5 {
            Some(self.collection.items.len())
        } else if self.page == 0 {
            list_source(self.tab, self.mine_detail).map(|source| self.lists[source].len())
        } else {
            None
        };
        if let Some(len) = video_len.filter(|_| !self.help) {
            if key == "v" {
                self.view = (self.view + 1) % 3;
                self.settings_changed = true;
                return;
            }
            let columns = video_columns(self.view) as i32;
            let step = match key {
                "UP" => Some(-columns),
                "DOWN" => Some(columns),
                "LEFT" => Some(-1),
                "RIGHT" => Some(1),
                _ => None,
            };
            if let Some(step) = step {
                let next = (self.sel + step).max(0);
                if next < len as i32 {
                    self.sel = next;
                }
                return;
            }
        }
        match self.page {
            0 => self.home_key(key),
            1 => self.play_key(key, repeat),
            _ => self.page_key(key),
        }
    }
}
