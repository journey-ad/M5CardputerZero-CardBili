use super::super::*;

impl App {
    pub(in crate::app) fn sync_home(&mut self) {
        if self.page != 0 && self.page != 5 {
            return;
        }

        // 选中项保持在当前页面范围内
        if self.page == 0 {
            let len = if self.tab == 4 {
                6
            } else if self.tab == 3 && self.mine_detail < 0 {
                3
            } else {
                self.lists[list_source(self.tab, self.mine_detail).unwrap()].len() as i32
            };
            self.sel = self.sel.clamp(0, (len - 1).max(0));

            // 视频列表接近底部时自动加载下一页
            if self.tab < 3
                && self.loaded[self.tab]
                && !self.loading_more[self.tab]
                && self.has_more[self.tab]
                && !self.lists[self.tab].is_empty()
                && self.sel as usize + video_columns(self.view) * 2 >= self.lists[self.tab].len()
            {
                self.loading_more[self.tab] = true;
                if self.tab == 2 {
                    tasks::load_dynamic(
                        self.dynamic_request,
                        self.dynamic_offset.clone(),
                        self.cookies.clone(),
                        self.tx.clone(),
                        false,
                    );
                } else {
                    tasks::load_more(
                        self.tab,
                        (self.pages[self.tab] + 1) as u32,
                        self.cookies.clone(),
                        self.tx.clone(),
                    );
                }
            }
            if self.tab == 3
                && self.mine_detail < 0
                && (self.home_dirty || self.home_view.is_some())
            {
                self.home_dirty = false;
                self.home_view = None;
                self.ui.set_history_count(if self.loaded[3] {
                    format!("{} 条", self.lists[3].len()).into()
                } else {
                    "".into()
                });
                self.ui.set_favorite_count(if self.loaded[4] {
                    format!("{} 条", self.lists[4].len()).into()
                } else {
                    "".into()
                });
            }
        }

        let uploader = self.page == 5;
        let source = if uploader {
            5
        } else if let Some(source) = list_source(self.tab, self.mine_detail) {
            source
        } else {
            return;
        };
        let content_width = if uploader || self.settings.menu_position == 0 {
            320.0
        } else {
            264.0
        };
        let content_height = if uploader || self.settings.menu_position == 0 {
            126.0
        } else {
            150.0
        };
        let columns = video_columns(self.view);
        let cell_height = if self.view == 2 { 113.0 } else { 84.0 };
        let context = (
            source as i32,
            self.view,
            self.page,
            self.settings.menu_position,
        );
        if self.scroll_context != Some(context) {
            if !self.go_back {
                self.first_row = 0;
                self.grid_scroll = 0.0;
            }
            self.scroll_context = Some(context);
        }
        let (items, covers) = if uploader {
            (&self.uploader.items, &self.uploader.covers)
        } else {
            (&self.lists[source], &self.covers[source])
        };
        self.sel = self.sel.clamp(0, (items.len() as i32 - 1).max(0));
        let selected = self.sel as usize;
        let (start, end, visible_count) = if self.view == 0 {
            let rows = (content_height / 30.0) as usize;
            if selected < self.first_row {
                self.first_row = selected;
            }
            if selected >= self.first_row + rows {
                self.first_row = selected + 1 - rows;
            }
            self.first_row = self.first_row.min(items.len().saturating_sub(rows));
            (
                self.first_row,
                (self.first_row + rows + 1).min(items.len()),
                rows,
            )
        } else {
            let selected_y = (selected / columns) as f64 * cell_height;
            if selected_y < self.grid_scroll {
                self.grid_scroll = selected_y;
            }
            if selected_y + cell_height > self.grid_scroll + content_height {
                self.grid_scroll = selected_y + cell_height - content_height;
            }
            let total_rows = items.len().div_ceil(columns);
            self.grid_scroll = self.grid_scroll.clamp(
                0.0,
                (total_rows as f64 * cell_height - content_height).max(0.0),
            );
            let first_row = (self.grid_scroll / cell_height) as usize;
            let start = first_row * columns;
            let rows = (content_height / cell_height).ceil() as usize;
            let end = (start + (rows + 1) * columns).min(items.len());
            self.ui
                .set_grid_offset((self.grid_scroll - first_row as f64 * cell_height) as f32);
            (start, end, rows * columns)
        };
        self.ui.set_first_row(self.first_row as i32);

        // 可视区内缺失的统计信息按需补全
        let stats_end = if self.view == 0 {
            end
        } else {
            ((self.grid_scroll + content_height) / cell_height).ceil() as usize * columns
        };
        for item in items.iter().take(stats_end).skip(start) {
            let missing = item.views.is_none()
                || if self.view == 0 {
                    item.likes.is_none()
                } else {
                    item.danmaku.is_none()
                };
            if missing
                && !self.stats_requested.contains(&item.bvid)
                && self
                    .stats_queue
                    .try_send((item.bvid.clone(), self.cookies.clone()))
                    .is_ok()
            {
                self.stats_requested.insert(item.bvid.clone());
            }
        }
        if uploader {
            if !self.uploader.loading
                && self.uploader.error.is_empty()
                && self.uploader.has_more
                && selected + visible_count >= items.len()
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
            self.ui
                .set_uploader_name(self.uploader.owner.name.as_str().into());
            self.ui.set_uploader_loading(self.uploader.loading);
            self.ui
                .set_uploader_error(self.uploader.error.as_str().into());
        }

        // 列表标题跑马灯
        let title_width = items
            .get(selected)
            .map(|item| util::est_w(&item.title, 12.0))
            .unwrap_or(0.0);
        self.ui
            .set_title_marquee(self.view == 0 && title_width > content_width - 16.0);
        self.ui
            .set_title_shift(yoyo_shift(title_width, content_width - 16.0, self.marq_t));

        let grid_start = if self.view == 0 { 0 } else { start };
        let current_view = (
            source,
            self.view,
            self.page,
            grid_start,
            self.settings.menu_position,
        );
        let dirty = if uploader {
            self.uploader_dirty
        } else {
            self.home_dirty
        };
        if dirty || self.home_view != Some(current_view) {
            self.home_view = Some(current_view);
            if uploader {
                self.uploader_dirty = false;
            } else {
                self.home_dirty = false;
            }
            if self.view == 0 {
                sync_list(&self.ui, items, &self.stats_cache);
            } else {
                let mut rows = Vec::with_capacity(end - start);
                let mut images = Vec::with_capacity(end - start);
                for (index, item) in items.iter().enumerate().take(end).skip(start) {
                    let mut row = video_row(item, &self.stats_cache);
                    row.views = compact_card_count(&row.views);
                    row.danmaku = compact_card_count(&row.danmaku);
                    rows.push(row);
                    images.push(
                        covers
                            .get(index)
                            .and_then(Clone::clone)
                            .unwrap_or_else(|| self.ph.clone()),
                    );
                }
                self.ui.set_cols(columns as i32);
                self.ui.set_cell_w((content_width / columns as f64) as f32);
                self.ui.set_cell_h(cell_height as f32);
                self.ui.set_grid_rows(ModelRc::new(VecModel::from(rows)));
                self.ui
                    .set_grid_covers(ModelRc::new(VecModel::from(images)));
            }
        }
        self.ui.set_grid_active(if selected < items.len() {
            self.sel - grid_start as i32
        } else {
            -1
        });
    }
}
