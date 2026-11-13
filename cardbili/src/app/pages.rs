use super::*;

impl App {
    pub(super) fn page_key(&mut self, k: &str) {
        if self.page == 7 {
            self.following_key(k);
        } else if self.page == 6 {
            self.comments_key(k);
        } else if self.page == 2 {
            match k {
                "ESC" | "BACKSPACE" => {
                    if self.login_uploader.take().is_some()
                        || self.login_return
                        || self.login_following
                    {
                        self.login_following = false;
                        self.login_return = false;
                        self.go_back = true;
                    } else {
                        self.page = 0;
                    }
                }
                "r" => {
                    self.ui.set_has_qr(false);
                    self.ui.set_qr_status("正在刷新二维码…".into());
                    self.login_request += 1;
                    tasks::start_login(self.login_request, self.tx.clone());
                }
                _ => {}
            }
        } else if self.page == 3 {
            match k {
                "ENTER" | "SPACE" => {
                    if !self.detail_bvid.is_empty() {
                        self.status = "正在加载视频…".into();
                        self.status_detail.clear();
                        self.requested_play = Some(self.detail_bvid.clone());
                    }
                }
                "u" => {
                    self.requested_uploader = self
                        .detail_display
                        .as_ref()
                        .map(|display| display.data.owner.clone())
                        .filter(|owner| !owner.mid.is_empty());
                }
                "ESC" | "BACKSPACE" => {
                    self.go_back = true;
                }
                _ => {}
            }
        } else if self.page == 4 {
            let limit = (self.ui.get_about_content_h() as f64 - 150.0).max(0.0);
            match k {
                "UP" | "f" | "w" | "PGUP" => {
                    self.about_scroll = (self.about_scroll - 20.0).max(0.0)
                }
                "DOWN" | "x" | "s" | "PGDN" => {
                    self.about_scroll = (self.about_scroll + 20.0).min(limit)
                }
                "ESC" | "BACKSPACE" | "ENTER" => self.page = 0,
                _ => {}
            }
        } else if self.page == 5 {
            match k {
                "ENTER" => {
                    self.requested_play = self
                        .uploader
                        .items
                        .get(self.sel as usize)
                        .map(|item| item.bvid.clone())
                }
                "SPACE" => {
                    self.requested_detail = self
                        .uploader
                        .items
                        .get(self.sel as usize)
                        .map(|item| (item.bvid.clone(), item.title.clone()))
                }
                "r" => {
                    self.uploader_request += 1;
                    self.uploader.loading = true;
                    self.uploader.error.clear();
                    tasks::load_uploader(
                        self.uploader.owner.mid.clone(),
                        if self.uploader.items.is_empty() {
                            1
                        } else {
                            self.uploader.page + 1
                        },
                        self.cookies.clone(),
                        self.uploader_request,
                        self.tx.clone(),
                    );
                }
                "ESC" | "BACKSPACE" => self.go_back = true,
                _ => {}
            }
        }
    }
}
