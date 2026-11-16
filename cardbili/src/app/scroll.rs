use slint::ComponentHandle as _;
use super::*;

impl App {
    pub(super) fn scroll_key(&mut self, key: &str) -> bool {
        let (direction, full_page) = match key {
            "UP" | "f" | "w" => (-1.0, false),
            "DOWN" | "x" | "s" => (1.0, false),
            "LEFT" | "z" | "PGUP" => (-1.0, true),
            "RIGHT" | "c" | "PGDN" => (1.0, true),
            _ => return false,
        };
        let height = if self.page == Page::Detail {
            self.ui.global::<crate::Detail>().get_viewport_h()
        } else {
            self.ui.global::<crate::Comments>().get_viewport_h()
        } as f64;
        let delta = direction * if full_page { height } else { 22.0 };
        if self.page == Page::Detail {
            self.detail_scroll = (self.detail_scroll + delta).clamp(0.0, self.detail_maxv);
            self.ui.global::<crate::Detail>().set_scroll(self.detail_scroll as f32);
            sync_thumb(&self.ui, self.detail_scroll, self.detail_maxv);
            if delta > 0.0
                && self.detail_scroll >= self.detail_maxv - 60.0
                && self.comment_page.has_more
                && self.comment_page.error.is_empty()
            {
                self.load_comments();
            }
        } else {
            self.scroll_comments(delta);
        }
        true
    }
}
