use super::super::*;

impl App {
    pub(in crate::app) fn home_key(&mut self, k: &str) {
        if let Some(source) = list_source(self.tab, self.mine_detail) {
            if !self.help && self.video_list_key(k, self.lists[source].len()) {
                return;
            }
        }
        if self.tab.is_video() && !self.help && (k == "ESC_DOWN" || k == "ESC_UP") {
            if k == "ESC_DOWN" {
                self.esc_press = Some(Instant::now());
                self.ui.set_bar("长按 2 秒退出".into());
                self.ui.set_bar_sticky(true);
                self.bar_until = None;
            } else {
                self.esc_press = None;
                self.bar_until = Some(Instant::now() + Duration::from_millis(3000));
            }
            return;
        }
        if k == "b" && !self.help {
            self.settings.menu_position = self.settings.menu_position.cycle(1);
            self.settings_changed = true;
            return;
        }
        if matches!(k, "TAB" | "BACKTAB") && !self.help {
            let step = if k == "TAB" { 1 } else { 4 };
            self.tab = HomeTab::from_index((self.tab.index() + step) % 5);
            self.back_stack.clear();
            self.direction_hold = None;
            self.ui.set_bar("".into());
            self.ui.set_bar_sticky(false);
            self.sel = 0;
            self.cache_checked = None;
            self.mine_detail = -1;
            if !self.tab.is_video() {
                self.loading = false;
                self.status.clear();
                self.status_detail.clear();
                self.acct_loaded = false;
            } else {
                if self.loaded[self.tab.index()] {
                    self.loading = false;
                    self.status = format!("{} 条", self.lists[self.tab.index()].len());
                    self.status_detail.clear();
                } else {
                    self.loading = true;
                    self.status = "加载中…".into();
                    self.status_detail.clear();
                }
                self.load_home_list(self.tab.index(), false);
                if self.loaded[self.tab.index()] && !self.covers_started[self.tab.index()] {
                    self.covers_started[self.tab.index()] = true;
                    self.covers[self.tab.index()] = vec![None; self.lists[self.tab.index()].len()];
                    tasks::spawn_covers(
                        self.tab.index(),
                        self.lists[self.tab.index()].clone(),
                        self.tx.clone(),
                    );
                }
            }
            return;
        }
        if self.help {
            if matches!(k, "h" | "ESC" | "BACKSPACE") {
                self.help = false;
            }
        } else if k == "h" {
            self.help = true;
        } else if k == "l" && self.tab != HomeTab::Settings {
            self.requested_login = true;
        } else if self.tab == HomeTab::Settings {
            self.settings_key(k);
        } else if self.tab == HomeTab::Mine {
            self.mine_key(k);
        } else {
            match k {
                "ENTER" => {
                    if let Some(it) = self.lists[self.tab.index()].get(self.sel as usize) {
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
                    if let Some(it) = self.lists[self.tab.index()].get(self.sel as usize) {
                        if !it.bvid.is_empty() {
                            self.requested_detail = Some((it.bvid.clone(), it.title.clone()));
                        }
                    }
                }
                "r" => {
                    self.loaded[self.tab.index()] = false;
                    self.loading = true;
                    self.status = "刷新中…".into();
                    self.status_detail.clear();
                    self.load_home_list(self.tab.index(), true);
                }
                "ESC_HOLD" => {
                    self.quit = true;
                }
                _ => {}
            }
        }
    }
}

impl App {
    pub(in crate::app) fn load_home_list(&mut self, source: usize, force: bool) {
        if source == 2 {
            self.dynamic_request += 1;
            self.loading_more[2] = false;
            tasks::load_dynamic(
                self.dynamic_request,
                String::new(),
                self.cookies.clone(),
                self.tx.clone(),
                force,
            );
        } else if source == 3 {
            self.history_request += 1;
            self.loading_more[3] = false;
            tasks::load_history(
                None,
                self.cookies.clone(),
                self.history_request,
                force,
                self.tx.clone(),
            );
        } else {
            tasks::load_list(source, self.cookies.clone(), self.tx.clone(), force);
        }
    }
}
