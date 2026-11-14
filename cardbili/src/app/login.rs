use slint::ComponentHandle as _;
use super::*;

impl App {
    pub(super) fn login_key(&mut self, k: &str) {
        match k {
            "ESC" | "BACKSPACE" => {
                self.login_uploader = None;
                self.login_following = false;
                self.login_favorites = false;
                self.login_return = false;
                self.go_back = true;
            }
            "r" => {
                self.ui.global::<crate::Login>().set_has_qr(false);
                self.ui.global::<crate::Login>().set_qr_status("正在刷新二维码…".into());
                self.login_request += 1;
                tasks::start_login(self.login_request, self.tx.clone());
            }
            _ => {}
        }
    }
}
