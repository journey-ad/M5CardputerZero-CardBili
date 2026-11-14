use super::*;

impl App {
    pub(super) fn scroll_key(&mut self, key: &str, repeat: bool) -> bool {
        let (direction, full_page) = match key {
            "UP" | "f" | "w" => (-1.0, false),
            "DOWN" | "x" | "s" => (1.0, false),
            "LEFT" | "z" | "PGUP" => (-1.0, true),
            "RIGHT" | "c" | "PGDN" => (1.0, true),
            _ => return false,
        };
        let height = if self.page == Page::Detail {
            self.ui.get_detail_viewport_h()
        } else {
            self.ui.get_comment_viewport_h()
        } as f64;
        let step = direction * 22.0;
        let delta = if full_page {
            self.scroll_tap = None;
            direction * height
        } else {
            let double = !repeat
                && self.scroll_tap.take().is_some_and(|(previous, since)| {
                    previous == step && since.elapsed() <= Duration::from_millis(300)
                });
            if double {
                direction * (height - 22.0)
            } else {
                step
            }
        };
        if self.page == Page::Detail {
            self.detail_scroll = (self.detail_scroll + delta).clamp(0.0, self.detail_maxv);
            self.ui.set_detail_scroll(self.detail_scroll as f32);
            sync_thumb(&self.ui, self.detail_scroll, self.detail_maxv);
        } else {
            self.scroll_comments(delta);
        }
        true
    }
}
