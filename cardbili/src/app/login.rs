use super::*;
use slint::ComponentHandle as _;

impl App {
    pub(super) fn start_login(&mut self, status: &str) {
        let login = self.ui.global::<crate::Login>();
        login.set_has_qr(false);
        login.set_qr_status(status.into());
        self.login_request += 1;
        tasks::start_login(self.login_request, self.tx.clone());
    }

    pub(super) fn clear_login_destination(&mut self) {
        self.login_uploader = None;
        self.login_following = false;
        self.login_favorites = false;
    }

    pub(super) fn qr_ready(&mut self, request: u64, url: String) {
        let login = self.ui.global::<crate::Login>();
        if request != self.login_request || self.page != Page::Login {
            return;
        }
        if let Some(img) = util::make_qr(&url) {
            login.set_qr(img);
            login.set_has_qr(true);
        }
        login.set_qr_status(crate::i18n::tr("请用 B 站 App 扫码").into());
    }

    pub(super) fn qr_status(&mut self, request: u64, s: String) {
        let login = self.ui.global::<crate::Login>();
        if request != self.login_request || self.page != Page::Login {
            return;
        }
        login.set_qr_status(s.as_str().into());
    }

    pub(super) fn login_finished(&mut self, request: u64, session: crate::net::session::Login) {
        let login = self.ui.global::<crate::Login>();
        if request != self.login_request || self.page != Page::Login {
            return;
        }
        let ck = match crate::net::session::accept(session) {
            Ok(cookies) => cookies,
            Err(error) => {
                login.set_qr_status(error.into());
                return;
            }
        };
        crate::diagnostics::write(format_args!("[登录] 登录成功，已保存账号会话"));
        self.mid = util::parse_mid(&ck);
        self.cookies = Some(ck);
        self.loaded[HomeTab::Live.index()] = false;
        self.live_list.request += 1;
        self.account_dirty = true;
        login.set_qr_status(crate::i18n::tr("登录成功").into());
        self.acct_loaded = false;
        self.acct = None;
        self.avatar_img = None;
        self.favorites = favorites::Favorites {
            request: self.favorites.request + 1,
            ..Default::default()
        };
        if self.login_favorites {
            self.login_favorites = false;
            self.requested_favorites = true;
            return;
        }
        if self.login_following {
            self.login_following = false;
            self.requested_following = true;
            return;
        }
        if let Some(owner) = self.login_uploader.take() {
            self.requested_uploader = Some(owner);
            return;
        }
        self.go_back = true;
    }

    pub(super) fn login_key(&mut self, k: &str) {
        match k {
            "ESC" | "BACKSPACE" => {
                self.clear_login_destination();
                self.go_back = true;
            }
            "r" => {
                self.start_login(crate::i18n::tr("正在刷新二维码…"));
            }
            _ => {}
        }
    }
}
