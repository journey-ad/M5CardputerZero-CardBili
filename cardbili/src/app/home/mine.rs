use super::super::*;

impl App {
    pub(in crate::app) fn mine_key(&mut self, k: &str) {
        if self.mine_detail < 0 {
            match k {
                "UP" | "f" | "w" | "LEFT" | "z" => {
                    self.sel = (self.sel - 1).max(0);
                }
                "DOWN" | "x" | "s" | "RIGHT" | "c" => {
                    if self.sel < 2 {
                        self.sel += 1;
                    }
                }
                "ENTER" => {
                    if self.sel == 0 {
                        self.requested_following = true;
                        return;
                    }
                    if self.sel == 1 {
                        self.requested_home_list = Some(3);
                    } else {
                        self.requested_favorites = true;
                    }
                }
                "r" => {
                    tasks::fetch_account(self.cookies.clone(), self.tx.clone(), true);
                    self.load_favorites(true);
                }
                "ESC" | "BACKSPACE" => {
                    self.tab = HomeTab::Recommended;
                    self.sel = 0;
                    self.mine_detail = -1;
                }
                _ => {}
            }
        } else {
            let src = self.mine_detail as usize;
            match k {
                "ENTER" => {
                    if let Some(it) = self.lists[src].get(self.sel as usize) {
                        if !it.bvid.is_empty() {
                            self.status = "正在加载视频…".into();
                            self.status_detail.clear();
                            self.requested_play = Some(it.bvid.clone());
                        } else {
                            self.status = "该条目无视频".into();
                            self.status_detail.clear();
                        }
                    }
                }
                "SPACE" => {
                    if let Some(it) = self.lists[src].get(self.sel as usize) {
                        if !it.bvid.is_empty() {
                            self.requested_detail = Some((it.bvid.clone(), it.title.clone()));
                        }
                    }
                }
                "r" => {
                    self.loaded[src] = false;
                    self.loading = true;
                    self.status = "刷新中…".into();
                    self.status_detail.clear();
                    self.load_home_list(src, true);
                }
                "ESC" | "BACKSPACE" => {
                    self.go_back = true;
                }
                _ => {}
            }
        }
    }
}
