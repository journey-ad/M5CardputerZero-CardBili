use super::*;

impl App {
    pub(super) fn handle_key(&mut self, mut k: String) {
        if let Ok(number @ 4..=8) = k.parse::<usize>() {
            let actions = menu::actions(
                self.page,
                self.tab,
                self.mine_detail,
                self.help,
                self.cookies.is_some(),
                self.settings.menu_position,
                if self.page == 6 {
                    !self.comment_page.error.is_empty()
                } else {
                    !self.uploader.error.is_empty()
                },
            );
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
                self.detail_hold = None;
            }
            return;
        }
        match self.page {
            0 => self.home_key(&k),
            1 => self.play_key(&k),
            _ => self.page_key(&k),
        }
    }
}
