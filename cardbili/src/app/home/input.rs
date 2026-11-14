use super::super::*;

impl App {
    pub(in crate::app) fn home_key(&mut self, k: &str) {
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
            match k {
                "UP" | "f" | "w" => self.sel = (self.sel - 1).max(0),
                "DOWN" | "x" | "s" => self.sel = (self.sel + 1).min(5),
                "LEFT" | "z" | "RIGHT" | "c" => {
                    let step = if matches!(k, "LEFT" | "z") { -1 } else { 1 };
                    if self.sel == 0 {
                        self.view = self.view.cycle(step);
                        self.settings_changed = true;
                    } else if self.sel == 1 {
                        self.toggle_danmaku();
                    } else if self.sel == 2 {
                        let opacity = (self.settings.danmaku_opacity + step * 10).clamp(0, 100);
                        self.settings_changed |= opacity != self.settings.danmaku_opacity;
                        self.settings.danmaku_opacity = opacity;
                    } else if self.sel == 3 {
                        self.settings.menu_position = self.settings.menu_position.cycle(step);
                        self.settings_changed = true;
                    } else if self.sel == 4 {
                        self.settings.record_history = !self.settings.record_history;
                        self.history.set_enabled(self.settings.record_history);
                        self.settings_changed = true;
                    }
                }
                "ENTER" if self.sel == 5 => {
                    self.requested_about = true;
                }
                "ESC" | "BACKSPACE" => {
                    self.tab = HomeTab::Recommended;
                    self.sel = 0;
                }
                _ => {}
            }
        } else if self.tab == HomeTab::Mine {
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
        } else {
            tasks::load_list(source, self.cookies.clone(), self.tx.clone(), force);
        }
    }
}
