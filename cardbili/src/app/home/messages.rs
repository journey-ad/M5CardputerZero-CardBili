use super::super::*;

impl App {
    pub(in crate::app) fn dynamic_loaded(
        &mut self,
        request: u64,
        offset: String,
        result: Result<model::DynamicPage, String>,
    ) {
        if request != self.dynamic_request || (!offset.is_empty() && offset != self.dynamic_offset)
        {
            return;
        }
        if !offset.is_empty() {
            self.loading_more[2] = false;
        }
        match result {
            Ok(page) => {
                if offset.is_empty() {
                    self.list_loaded(2, page.items);
                } else {
                    self.list_more(2, page.items);
                }
                if !offset.is_empty() || self.pages[2] <= 1 && !self.loading_more[2] {
                    self.dynamic_offset = page.offset;
                    self.has_more[2] = page.has_more;
                }
                if self.lists[2].is_empty() && page.has_more {
                    self.loading_more[2] = true;
                    tasks::load_dynamic(
                        self.dynamic_request,
                        self.dynamic_offset.clone(),
                        self.cookies.clone(),
                        self.tx.clone(),
                        false,
                    );
                }
            }
            Err(error) => {
                self.has_more[2] = false;
                self.list_error(2, error);
            }
        }
    }

    pub(in crate::app) fn history_loaded(
        &mut self,
        request: u64,
        first: bool,
        result: Result<model::HistoryPage, String>,
    ) {
        if request != self.history_request {
            return;
        }
        if !first {
            self.loading_more[3] = false;
        }
        match result {
            Ok(page) => {
                let update_cursor = !first || self.pages[3] <= 1 && !self.loading_more[3];
                if first {
                    self.list_loaded(3, page.items);
                } else {
                    self.list_more(3, page.items);
                }
                if update_cursor {
                    self.history_cursor = page.cursor;
                    self.has_more[3] = page.has_more;
                }
            }
            Err(error) => {
                self.loading_more[3] = false;
                self.has_more[3] = false;
                self.list_error(3, error);
            }
        }
    }

    pub(in crate::app) fn list_loaded(&mut self, source: usize, items: Vec<model::Item>) {
        self.home_dirty = true;
        let selection = refresh_first_page(
            &mut self.lists[source],
            &mut self.covers[source],
            &mut self.first_page_sizes[source],
            items,
            self.sel,
        );
        self.loaded[source] = true;
        if self.pages[source] <= 1 {
            self.has_more[source] = true;
        }
        self.covers_started[source] = true;
        let len = self.lists[source].len();
        let missing = self.lists[source]
            .iter()
            .zip(&self.covers[source])
            .filter(|(_, cover)| cover.is_none())
            .map(|(item, _)| item.clone())
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            tasks::spawn_covers(source, missing, self.tx.clone());
        }
        if list_source(self.tab, self.mine_detail) == Some(source) {
            self.loading = false;
        }
        if self.page == Page::Home && list_source(self.tab, self.mine_detail) == Some(source) {
            self.sel = selection;
            self.status = if len == 0 {
                if source == crate::HomeTab::Live.index() { "暂无直播".into() } else { "暂无内容".into() }
            } else if source == crate::HomeTab::Live.index() {
                format!("{} 个直播间", len)
            } else {
                format!("{} 条", len)
            };
            self.status_detail.clear();
            if let Ok(v) = std::env::var("BILI_SEL") {
                if let Ok(v) = v.parse::<i32>() {
                    self.sel = v;
                }
            }
        }
    }

    pub(in crate::app) fn list_more(&mut self, source: usize, items: Vec<model::Item>) {
        self.home_dirty = true;
        self.loading_more[source] = false;
        if items.is_empty() {
            self.has_more[source] = false;
            return;
        }
        let mut known: HashSet<String> = self.lists[source]
            .iter()
            .map(|item| item.bvid.clone())
            .collect();
        let added: Vec<model::Item> = items
            .into_iter()
            .filter(|item| known.insert(item.bvid.clone()))
            .collect();
        if added.is_empty() {
            self.has_more[source] = false;
            return;
        }
        self.pages[source] += 1;
        self.lists[source].extend(added.clone());
        self.covers[source].extend(vec![None; added.len()]);
        if std::env::var("BILI_DUMP").is_ok() {
            eprintln!(
                "MORE tab={} page={} add={} total={}",
                source,
                self.pages[source],
                added.len(),
                self.lists[source].len()
            );
        }
        tasks::spawn_covers(source, added, self.tx.clone());
        if source == self.tab.index() && self.page == Page::Home {
            self.status = if source == crate::HomeTab::Live.index() {
                format!("{} 个直播间", self.lists[source].len())
            } else {
                format!("{} 条", self.lists[source].len())
            };
            self.status_detail.clear();
        }
    }

    pub(in crate::app) fn list_error(&mut self, source: usize, msg: String) {
        let g_status = self.ui.global::<crate::Status>();
        let (friendly, tech) = match msg.split_once('\n') {
            Some((a, b)) => (a.to_string(), b.to_string()),
            None => (msg, String::new()),
        };
        if source == 9 {
            self.status = friendly;
            self.status_detail = tech;
        } else if list_source(self.tab, self.mine_detail) == Some(source) {
            self.loading = false;
            if self.page == Page::Home {
                self.status = friendly;
                // 仅有技术细节时视为接口失败，未登录等提示只显示空状态
                if !tech.is_empty() {
                    g_status.set_bar("加载失败".into());
                    g_status.set_bar_sticky(false);
                }
                self.status_detail = tech;
            }
        }
    }

    pub(in crate::app) fn cover_loaded(
        &mut self,
        source: usize,
        bvid: String,
        (w, h, raw): crate::cache::Pixels,
    ) {
        let (items, covers, dirty) = match source {
            0..super::COLLECTION_SOURCE => (
                &self.lists[source],
                &mut self.covers[source],
                &mut self.home_dirty,
            ),
            super::COLLECTION_SOURCE => (
                &self.collection.items,
                &mut self.collection.covers,
                &mut self.collection_dirty,
            ),
            _ => return,
        };
        let Some(index) = items.iter().position(|item| item.bvid == bvid) else {
            return;
        };
        if let Some(cover) = covers.get_mut(index) {
            *cover = Some(util::raw_to_image(w, h, &raw));
            *dirty = true;
        }
    }
}
