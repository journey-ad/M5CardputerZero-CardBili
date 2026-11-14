use slint::ComponentHandle as _;
use super::super::*;
use slint::Model;

impl App {
    pub(in crate::app) fn sync_home(&mut self) {
        if self.page != Page::Home && self.page != Page::Collection {
            return;
        }
        if self.page == Page::Collection {
            self.sync_collection_header();
        }

        // 选中项保持在当前页面范围内
        if self.page == Page::Home {
            let len = if self.tab == HomeTab::Settings {
                let appearance = self.ui.global::<crate::Preferences>().get_appearance_items().row_count() as i32;
                let general = self.ui.global::<crate::Preferences>().get_general_items().row_count() as i32;
                appearance + general
            } else if self.tab == HomeTab::Mine && self.mine_detail < 0 {
                3
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
                self.ui.global::<crate::ListModel>().set_history_count(if self.loaded[3] {
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
            5
        } else if let Some(source) = list_source(self.tab, self.mine_detail) {
            source
        } else {
            return;
        };
        let content_width = if collection || self.settings.menu_position == MenuPosition::Top {
            320.0
        } else {
            264.0
        };
        let content_height =
            if collection && matches!(self.collection.source, model::VideoSource::Author(_)) {
                114.0
            } else if collection || self.settings.menu_position == MenuPosition::Top {
                126.0
            } else {
                150.0
            };
        let columns = video_columns(self.view);
        let cell_height = if self.view == VideoLayout::TwoColumns {
            113.0
        } else {
            84.0
        };
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
        let (items, covers) = if collection {
            (&self.collection.items, &self.collection.covers)
        } else {
            (&self.lists[source], &self.covers[source])
        };
        self.sel = self.sel.clamp(0, (items.len() as i32 - 1).max(0));
        let selected = self.sel as usize;
        let (start, end, visible_count) = if self.view == VideoLayout::List {
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
            self.ui.global::<crate::ListModel>().set_grid_offset((self.grid_scroll - first_row as f64 * cell_height) as f32);
            (start, end, rows * columns)
        };
        self.ui.global::<crate::ListModel>().set_first_row(self.first_row as i32);

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
            self.ui.global::<crate::Collection>().set_loading(self.collection.loading);
            self.ui.global::<crate::Collection>().set_error(self.collection.error.as_str().into());
        }

        // 列表标题跑马灯
        let title_width = items
            .get(selected)
            .map(|item| util::est_w(&item.title, 12.0))
            .unwrap_or(0.0);
        self.ui.global::<crate::ListModel>().set_title_marquee(
            self.view == VideoLayout::List && title_width > content_width - 16.0,
        );
        self.ui.global::<crate::ListModel>().set_title_shift(yoyo_shift(title_width, content_width - 16.0, self.marq_t));

        let grid_start = if self.view == VideoLayout::List {
            0
        } else {
            start
        };
        let current_view = (
            source,
            self.view,
            self.page,
            grid_start,
            self.settings.menu_position,
        );
        let dirty = if collection {
            self.collection_dirty
        } else {
            self.home_dirty
        };
        if dirty || self.home_view != Some(current_view) {
            self.home_view = Some(current_view);
            if collection {
                self.collection_dirty = false;
            } else {
                self.home_dirty = false;
            }
            if self.view == VideoLayout::List {
                sync_list(&self.ui, items);
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
                self.ui.global::<crate::ListModel>().set_cols(columns as i32);
                self.ui.global::<crate::ListModel>().set_cell_w((content_width / columns as f64) as f32);
                self.ui.global::<crate::ListModel>().set_cell_h(cell_height as f32);
                self.ui.global::<crate::ListModel>().set_grid_rows(ModelRc::new(VecModel::from(rows)));
                self.ui.global::<crate::ListModel>().set_grid_covers(ModelRc::new(VecModel::from(images)));
            }
        }
        self.ui.global::<crate::ListModel>().set_grid_active(if selected < items.len() {
            self.sel - grid_start as i32
        } else {
            -1
        });
    }
}
