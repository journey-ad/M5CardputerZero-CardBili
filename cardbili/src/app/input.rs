use super::*;

impl App {
    pub(super) fn handle_key(&mut self, mut k: String) {
        if let Ok(number @ 4..=8) = k.parse::<usize>() {
            let actions = self.menu_actions();
            let Some(action) = actions[number - 4] else {
                return;
            };
            k = action.key().to_string();
        }
        self.last_input = Instant::now();
        if k == "LEFT_UP" {
            self.left_hold = None;
            return;
        }
        if k == "RIGHT_UP" {
            self.right_hold = None;
            return;
        }
        if k == "UP_UP" || k == "DOWN_UP" {
            let direction = if k == "UP_UP" { -22.0 } else { 22.0 };
            if self.detail_hold.is_some_and(|(step, _)| step == direction) {
                let (_, since) = self.detail_hold.take().unwrap();
                if since.elapsed() <= Duration::from_millis(300) {
                    self.scroll_tap = Some((direction, since));
                }
            }
            return;
        }
        if matches!(self.page, 3 | 6) && self.scroll_key(&k) {
            return;
        }
        let video_len = if self.page == 5 {
            Some(self.uploader.items.len())
        } else if self.page == 0 {
            list_source(self.tab, self.mine_detail).map(|source| self.lists[source].len())
        } else {
            None
        };
        if let Some(len) = video_len.filter(|_| !self.help) {
            if k == "v" {
                self.view = (self.view + 1) % 3;
                self.settings_changed = true;
                return;
            }
            let columns = video_columns(self.view) as i32;
            let step = match k.as_str() {
                "UP" | "f" | "w" => Some(-columns),
                "DOWN" | "x" | "s" => Some(columns),
                "LEFT" | "z" => Some(-1),
                "RIGHT" | "c" => Some(1),
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
            0 => self.home_key(&k),
            1 => self.play_key(&k),
            _ => self.page_key(&k),
        }
    }
}
