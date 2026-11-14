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
        if self.account_dirty {
            self.ui.set_acct_known(self.cookies.is_some());
            self.ui
                .set_acct_logged(self.acct.as_ref().map(|a| a.logged).unwrap_or(false));
            self.ui.set_acct_name(
                self.acct
                    .as_ref()
                    .map(|a| a.name.as_str())
                    .unwrap_or_default()
                    .into(),
            );
            self.ui.set_acct_mid(
                self.acct
                    .as_ref()
                    .map(|a| a.mid.as_str())
                    .unwrap_or_default()
                    .into(),
            );
            let lv = self.acct.as_ref().map(|a| a.level).unwrap_or(0);
            self.ui.set_acct_level(
                if lv > 0 {
                    format!("Lv{}", lv)
                } else {
                    String::new()
                }
                .as_str()
                .into(),
            );
            self.ui.set_acct_vip(
                self.acct
                    .as_ref()
                    .map(|a| a.vip.as_str())
                    .unwrap_or_default()
                    .into(),
            );
            self.ui.set_has_avatar(self.avatar_img.is_some());
            if let Some(im) = self.avatar_img.clone() {
                self.ui.set_avatar(im);
            }

            self.account_dirty = false;
        }
    }
}
