use slint::ComponentHandle as _;
use super::super::*;
use slint::Model;

impl App {
    pub(in crate::app) fn sync_home(&mut self) {
        let preferences = self.ui.global::<crate::Preferences>();
        let list_model = self.ui.global::<crate::ListModel>();
        let g_collection = self.ui.global::<crate::Collection>();
        if self.page != Page::Home && self.page != Page::Collection {
            return;
        }
        if self.page == Page::Collection {
            self.sync_collection_header();
        }

        // 选中项保持在当前页面范围内
        if self.page == Page::Home {
            let len = if self.tab == HomeTab::Settings {
                let appearance = preferences.get_appearance_items().row_count() as i32;
                let general = preferences.get_general_items().row_count() as i32;
                appearance + general
            } else if self.tab == HomeTab::Mine && self.mine_detail < 0 {
                4
            } else {
                self.lists[list_source(self.tab, self.mine_detail).unwrap()].len() as i32
            };
            self.sel = self.sel.clamp(0, (len - 1).max(0));

            // 视频列表接近底部时自动加载下一页
            if self.tab.is_video()
                && self.loaded[self.tab.index()]
                && !self.loading_more[self.tab.index()]
                && self.has_more[self.tab.index()]
                && !self.lists[self.tab.index()].is_empty()
                && self.sel as usize + video_columns(self.view) * 2
                    >= self.lists[self.tab.index()].len()
            {
                self.loading_more[self.tab.index()] = true;
                if self.tab == HomeTab::Dynamic {
                    tasks::load_dynamic(
                        self.dynamic_request,
                        self.dynamic_offset.clone(),
                        self.cookies.clone(),
                        self.tx.clone(),
                        false,
                    );
                } else if self.tab == HomeTab::Live {
                    tasks::load_live_list(
                        self.live_list.request,
                        self.live_list.filter.clone(),
                        (self.pages[self.tab.index()] + 1) as u32,
                        self.tx.clone(),
                        false,
                    );
                } else {
                    tasks::load_more(
                        self.tab.index(),
                        (self.pages[self.tab.index()] + 1) as u32,
                        self.cookies.clone(),
                        self.tx.clone(),
                    );
                }
            }
            if self.tab == HomeTab::Mine
                && self.mine_detail == 3
                && self.has_more[3]
                && self.loaded[3]
                && !self.loading_more[3]
                && self.sel as usize + video_columns(self.view) * 2 >= self.lists[3].len()
            {
                self.loading_more[3] = true;
                tasks::load_history(
                    Some(self.history_cursor.clone()),
                    self.cookies.clone(),
                    self.history_request,
                    true,
                    self.tx.clone(),
                );
            }
            if self.tab == HomeTab::Mine
                && self.mine_detail < 0
                && (self.home_dirty || self.home_view.is_some())
            {
                self.home_dirty = false;
                self.home_view = None;
                list_model.set_history_count(if self.loaded[3] {
                    format!(
                        "{}{} 条",
                        self.lists[3].len(),
                        if self.has_more[3] { "+" } else { "" }
                    )
                    .into()
                } else {
                    "".into()
                });
            }
        }

        let collection = self.page == Page::Collection;
        let source = if collection {
            super::COLLECTION_SOURCE
        } else if let Some(source) = list_source(self.tab, self.mine_detail) {
            source
        } else {
            return;
        };
        let geometry = self.ui.global::<crate::Layout>();
        let content_width = geometry.get_video_width() as f64;
        let content_height = geometry.get_video_height() as f64;
        let row_height = list_model.get_row_height() as f64;
        let history = !collection && self.tab == HomeTab::Mine && self.mine_detail == 3;
        let layout = if history { VideoLayout::List } else { self.view };
        let columns = video_columns(layout);
        let mut cell_height = if layout == VideoLayout::TwoColumns {
            113.0
        } else {
            84.0
        };
        let context = (
            source as i32,
            layout,
            self.page,
            self.settings.menu_position,
        );
        if self.scroll_context != Some(context) {
            if !self.go_back {
                self.list_scroll = 0.0;
                self.grid_scroll = 0.0;
            }
            self.row_tops.clear();
            self.scroll_context = Some(context);
        }
        let (items, covers) = if collection {
            (&self.collection.items, &self.collection.covers)
        } else {
            (&self.lists[source], &self.covers[source])
        };
        let has_stats = items.iter().any(|item| {
            item.views.as_deref().is_some_and(|v| !v.is_empty())
                || item.danmaku.as_deref().is_some_and(|v| !v.is_empty())
        });
        if !has_stats {
            cell_height -= 12.0;
        }
        self.sel = self.sel.clamp(0, (items.len() as i32 - 1).max(0));
        let selected = self.sel as usize;
        let dirty = if collection {
            self.collection_dirty
        } else {
            self.home_dirty
        };
        let (start, end, visible_count) = if layout == VideoLayout::List {
            let tops: Vec<f64> = if !dirty && self.row_tops.len() == items.len() {
                self.row_tops.iter().map(|v| *v as f64).collect()
            } else if history {
                build_history_rows(items, row_height as f32)
                    .tops
                    .iter()
                    .map(|v| *v as f64)
                    .collect()
            } else {
                (0..items.len()).map(|i| i as f64 * row_height).collect()
            };
            // 分组首项的可见范围包含前面的日期标题
            let sel_top = selected
                .checked_sub(1)
                .map(|i| tops[i] + row_height)
                .unwrap_or(0.0);
            let sel_bottom = tops
                .get(selected)
                .map(|top| top + row_height)
                .unwrap_or(0.0);
            let total = tops.last().map(|v| v + row_height).unwrap_or(0.0);
            self.list_scroll = scroll_to_visible(
                self.list_scroll,
                sel_top,
                sel_bottom,
                content_height,
                total,
            );
            list_model.set_content_height(total as f32);
            list_model.set_scroll_y(self.list_scroll as f32);
            let rows = ((content_height / row_height).ceil() as usize + 1).min(items.len().max(1));
            (0, items.len(), rows)
        } else {
            let selected_y = (selected / columns) as f64 * cell_height;
            let total_rows = items.len().div_ceil(columns);
            self.grid_scroll = scroll_to_visible(
                self.grid_scroll,
                selected_y,
                selected_y + cell_height,
                content_height,
                total_rows as f64 * cell_height,
            );
            let first_row = (self.grid_scroll / cell_height) as usize;
            let start = first_row * columns;
            let rows = (content_height / cell_height).ceil() as usize;
            let end = (start + (rows + 1) * columns).min(items.len());
            list_model.set_grid_offset((self.grid_scroll - first_row as f64 * cell_height) as f32);
            (start, end, rows * columns)
        };

        if collection {
            if !self.collection.loading
                && self.collection.error.is_empty()
                && self.collection.has_more
                && selected + visible_count >= items.len()
            {
                self.collection.loading = true;
                self.collection.pending_page = self.collection.page + 1;
                tasks::load_collection(
                    self.collection.source.clone(),
                    self.collection.page + 1,
                    self.cookies.clone(),
                    self.collection_request,
                    self.tx.clone(),
                );
            }
            g_collection.set_loading(self.collection.loading);
            g_collection.set_error(self.collection.error.as_str().into());
        }

        // 列表标题跑马灯
        let title_width = items
            .get(selected)
            .map(|item| util::est_w(&item.title, 12.0))
            .unwrap_or(0.0);
        list_model.set_title_marquee(
            layout == VideoLayout::List && title_width > content_width - 16.0,
        );
        list_model.set_title_shift(yoyo_shift(title_width, content_width - 16.0, self.marq_t));

        let grid_start = if layout == VideoLayout::List {
            0
        } else {
            start
        };
        let current_view = (
            source,
            layout,
            self.page,
            grid_start,
            self.settings.menu_position,
        );
        if dirty || self.home_view != Some(current_view) {
            self.home_view = Some(current_view);
            if collection {
                self.collection_dirty = false;
            } else {
                self.home_dirty = false;
            }
            if layout == VideoLayout::List {
                if history {
                    let prepared = build_history_rows(items, row_height as f32);
                    self.row_tops = prepared.tops.clone();
                    sync_list(&self.ui, prepared.rows, prepared.headers, prepared.tops);
                } else {
                    let rows: Vec<VideoRow> = items.iter().map(video_row).collect();
                    let tops: Vec<f32> = (0..items.len())
                        .map(|i| i as f32 * row_height as f32)
                        .collect();
                    self.row_tops = tops.clone();
                    sync_list(&self.ui, rows, Vec::new(), tops.clone());
                }
            } else {
                let mut rows = Vec::with_capacity(end - start);
                let mut images = Vec::with_capacity(end - start);
                for (index, item) in items.iter().enumerate().take(end).skip(start) {
                    let mut row = video_row(item);
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
                list_model.set_cols(columns as i32);
                list_model.set_cell_w((content_width / columns as f64) as f32);
                list_model.set_cell_h(cell_height as f32);
                list_model.set_grid_rows(ModelRc::new(VecModel::from(rows)));
                list_model.set_grid_covers(ModelRc::new(VecModel::from(images)));
            }
        }
        list_model.set_grid_active(if selected < items.len() {
            self.sel - grid_start as i32
        } else {
            -1
        });
    }
}
