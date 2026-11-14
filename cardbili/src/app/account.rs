use slint::ComponentHandle as _;
use super::*;

impl App {
    pub(super) fn sync_account(&mut self) {
        // --- 账号信息 ---
        if self.tab == HomeTab::Mine && !self.acct_loaded {
            self.acct_loaded = true;
            if self.cookies.is_some() {
                tasks::fetch_account(self.cookies.clone(), self.tx.clone(), false);
                self.load_favorites(false);
            } else {
                self.acct = None;
                self.avatar_img = None;
                self.account_dirty = true;
            }
        }
        let account = self.ui.global::<crate::Account>();
        if self.account_dirty {
            account.set_known(self.cookies.is_some());
            account.set_logged(self.acct.as_ref().map(|a| a.logged).unwrap_or(false));
            account.set_name(
                self.acct
                    .as_ref()
                    .map(|a| a.name.as_str())
                    .unwrap_or_default()
                    .into(),
            );
            account.set_mid(
                self.acct
                    .as_ref()
                    .map(|a| a.mid.as_str())
                    .unwrap_or_default()
                    .into(),
            );
            let lv = self.acct.as_ref().map(|a| a.level).unwrap_or(0);
            account.set_level(
                if lv > 0 {
                    format!("Lv{}", lv)
                } else {
                    String::new()
                }
                .as_str()
                .into(),
            );
            account.set_vip(
                self.acct
                    .as_ref()
                    .map(|a| a.vip.as_str())
                    .unwrap_or_default()
                    .into(),
            );
            account.set_has_avatar(self.avatar_img.is_some());
            if let Some(im) = self.avatar_img.clone() {
                account.set_avatar(im);
            }

            self.account_dirty = false;
        }
    }
}
