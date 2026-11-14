use super::*;

impl App {
    pub(super) fn about_key(&mut self, k: &str) {
        let limit = (self.ui.get_about_content_h() as f64 - 150.0).max(0.0);
        match k {
            "UP" | "f" | "w" | "PGUP" => self.about_scroll = (self.about_scroll - 20.0).max(0.0),
            "DOWN" | "x" | "s" | "PGDN" => {
                self.about_scroll = (self.about_scroll + 20.0).min(limit)
            }
            "ESC" | "BACKSPACE" | "ENTER" => self.go_back = true,
            _ => {}
        }
    }
}
