use super::super::*;

impl App {
    pub(in crate::app) fn sync_home(&mut self) {
        // --- 选中项夹取 ---
        if self.page == 0 {
            let len = if self.tab == 4 {
                5
            } else if self.tab == 3 && self.mine_detail < 0 {
                2
            } else {
                self.lists[if self.tab == 3 {
                    self.mine_detail as usize
                } else {
                    self.tab
                }]
                .len() as i32
            };
            if len > 0 && self.sel >= len {
                self.sel = len - 1;
            }
            if self.sel < 0 {
                self.sel = 0;
            }
            // 推荐和热门列表接近底部时自动加载下一页
            if self.tab < 2
                && self.mine_detail < 0
                && self.view >= 0
                && self.loaded[self.tab]
                && !self.loading_more[self.tab]
                && self.has_more[self.tab]
            {
                let n = self.lists[self.tab].len() as i32;
                let cols2: i32 = if self.view == 2 {
                    2
                } else if self.view == 1 {
                    3
                } else {
                    1
                };
                if n > 0 && self.sel >= n - cols2 * 2 {
                    self.loading_more[self.tab] = true;
                    tasks::load_more(
                        self.tab,
                        (self.pages[self.tab] + 1) as u32,
                        self.cookies.clone(),
                        self.tx.clone(),
                    );
                }
            }
        }

        let content_width = if self.page == 5 || self.settings.menu_position == 0 {
            320.0
        } else {
            264.0
        };
        let content_height = if self.page == 5 || self.settings.menu_position == 0 {
            126.0
        } else {
            150.0
        };
        let visible_rows = (content_height / 30.0) as usize;
        let context = (
            if self.page == 5 { 5 } else { self.tab as i32 },
            self.view,
            self.mine_detail,
            self.settings.menu_position,
        );
        if self.scroll_context != Some(context) {
            if !self.go_back {
                self.first_row = 0;
                self.grid_scroll = 0.0;
            }
            self.scroll_context = Some(context);
        }
        let list_items = if self.page == 5 {
            Some(self.uploader.items.as_slice())
        } else if self.page == 0 && (self.view == 0 || self.tab == 3) {
            list_source(self.tab, self.mine_detail).map(|source| self.lists[source].as_slice())
        } else {
            None
        };
        if let Some(items) = list_items {
            self.sel = self.sel.clamp(0, (items.len() as i32 - 1).max(0));
            let selected = self.sel as usize;
            if selected < self.first_row {
                self.first_row = selected;
            }
            if selected >= self.first_row + visible_rows {
                self.first_row = selected + 1 - visible_rows;
            }
            self.first_row = self.first_row.min(items.len().saturating_sub(visible_rows));
            for item in items.iter().skip(self.first_row).take(visible_rows + 1) {
                if (item.views.is_none() || item.likes.is_none())
                    && !self.stats_requested.contains(&item.bvid)
                    && self
                        .stats_queue
                        .try_send((item.bvid.clone(), self.cookies.clone()))
                        .is_ok()
                {
                    self.stats_requested.insert(item.bvid.clone());
                }
            }
        }
        self.ui.set_first_row(self.first_row as i32);
        if self.page == 5 {
            if !self.uploader.loading
                && self.uploader.error.is_empty()
                && self.uploader.has_more
                && self.sel as usize + visible_rows >= self.uploader.items.len()
            {
                self.uploader.loading = true;
                tasks::load_uploader(
                    self.uploader.owner.mid.clone(),
                    self.uploader.page + 1,
                    self.cookies.clone(),
                    self.uploader_request,
                    self.tx.clone(),
                );
            }
            if self.uploader_dirty {
                sync_list(&self.ui, &self.uploader.items, &self.stats_cache);
                self.uploader_dirty = false;
            }
            self.ui
                .set_uploader_name(self.uploader.owner.name.as_str().into());
            self.ui.set_uploader_loading(self.uploader.loading);
            self.ui
                .set_uploader_error(self.uploader.error.as_str().into());
        }

        // 列表标题跑马灯
        let mut list_marq = false;
        let mut list_shift = 0.0f32;
        if self.page == 5 || (self.page == 0 && self.tab < 4 && (self.view == 0 || self.tab == 3)) {
            let sel_title: String = if self.page == 5 {
                self.uploader
                    .items
                    .get(self.sel as usize)
                    .map(|item| item.title.clone())
                    .unwrap_or_default()
            } else if self.tab == 3 {
                if self.mine_detail >= 0 {
                    self.lists[self.mine_detail as usize]
                        .get(self.sel as usize)
                        .map(|it| it.title.clone())
                        .unwrap_or_default()
                } else {
                    String::new()
                }
            } else {
                self.lists[self.tab]
                    .get(self.sel as usize)
                    .map(|it| it.title.clone())
                    .unwrap_or_default()
            };
            let est = util::est_w(&sel_title, 12.0);
            let clip = content_width - 16.0;
            list_marq = est > clip;
            list_shift = yoyo_shift(est, clip, self.marq_t);
        }
        self.ui.set_title_marquee(list_marq);
        self.ui.set_title_shift(list_shift);

        let grid_columns = if self.view == 2 { 2 } else { 3 };
        let cell_height = if self.view == 2 { 113.0 } else { 84.0 };
        if self.page == 0 && self.tab < 3 && self.view > 0 {
            let selected_y = (self.sel.max(0) as usize / grid_columns) as f64 * cell_height;
            if selected_y < self.grid_scroll {
                self.grid_scroll = selected_y;
            }
            if selected_y + cell_height > self.grid_scroll + content_height {
                self.grid_scroll = selected_y + cell_height - content_height;
            }
            let total_rows = self.lists[self.tab].len().div_ceil(grid_columns);
            self.grid_scroll = self.grid_scroll.clamp(
                0.0,
                (total_rows as f64 * cell_height - content_height).max(0.0),
            );
        }
        let grid_first_row = (self.grid_scroll / cell_height) as usize;
        let grid_start = grid_first_row * grid_columns;
        let grid_size = ((content_height / cell_height).ceil() as usize + 1) * grid_columns;
        if self.page == 0 && self.tab < 3 && self.view > 0 {
            let visible_end =
                ((self.grid_scroll + content_height) / cell_height).ceil() as usize * grid_columns;
            for item in self.lists[self.tab]
                .iter()
                .take(visible_end)
                .skip(grid_start)
            {
                if (item.views.is_none() || item.danmaku.is_none())
                    && !self.stats_requested.contains(&item.bvid)
                    && self
                        .stats_queue
                        .try_send((item.bvid.clone(), self.cookies.clone()))
                        .is_ok()
                {
                    self.stats_requested.insert(item.bvid.clone());
                }
            }
        }
        let current_home_view = (
            self.tab,
            self.view,
            self.mine_detail,
            grid_start,
            self.settings.menu_position,
        );
        self.ui
            .set_grid_offset((self.grid_scroll - grid_first_row as f64 * cell_height) as f32);
        if self.page == 0
            && self.tab < 4
            && (self.home_dirty || self.home_view != Some(current_home_view))
        {
            self.home_dirty = false;
            self.home_view = Some(current_home_view);
            if self.tab == 3 {
                if self.mine_detail < 0 {
                    self.ui.set_history_count(
                        if self.loaded[3] {
                            format!("{} 条", self.lists[3].len())
                        } else {
                            String::new()
                        }
                        .into(),
                    );
                    self.ui.set_favorite_count(
                        if self.loaded[4] {
                            format!("{} 条", self.lists[4].len())
                        } else {
                            String::new()
                        }
                        .into(),
                    );
                } else {
                    let src = self.mine_detail as usize;
                    sync_list(&self.ui, &self.lists[src], &self.stats_cache);
                }
            } else if self.view == 0 {
                sync_list(&self.ui, &self.lists[self.tab], &self.stats_cache);
            } else {
                let len = self.lists[self.tab].len();
                let cols = grid_columns as i32;
                let start = grid_start;
                let end = (start + grid_size).min(len);
                let mut rows: Vec<VideoRow> = Vec::new();
                let mut gc: Vec<slint::Image> = Vec::new();
                for (i, item) in self.lists[self.tab]
                    .iter()
                    .enumerate()
                    .take(end)
                    .skip(start)
                {
                    let mut row = video_row(item, &self.stats_cache);
                    row.views = compact_card_count(&row.views);
                    row.danmaku = compact_card_count(&row.danmaku);
                    rows.push(row);
                    let im = self.covers[self.tab]
                        .get(i)
                        .and_then(|o| o.clone())
                        .unwrap_or_else(|| self.ph.clone());
                    gc.push(im);
                }
                self.ui.set_cols(cols);
                self.ui.set_cell_w((content_width / cols as f64) as f32);
                self.ui.set_cell_h(cell_height as f32);
                self.ui.set_grid_rows(ModelRc::new(VecModel::from(rows)));
                self.ui
                    .set_grid_covers(slint::ModelRc::new(slint::VecModel::from(gc)));
            }
        }
        if self.page == 0 && self.tab < 3 && self.view > 0 {
            let active = if (self.sel as usize) < self.lists[self.tab].len() {
                self.sel - grid_start as i32
            } else {
                -1
            };
            self.ui.set_grid_active(active);
        }
    }
}
