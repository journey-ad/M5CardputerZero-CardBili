use super::super::*;
use slint::ComponentHandle as _;

#[derive(Default)]
pub(in crate::app) struct LiveState {
    pub areas: Vec<model::LiveArea>,
    pub filter: model::LiveArea,
    pub request: u64,
    parent: Option<u64>,
}

impl LiveState {
    fn options(&self) -> Vec<&model::LiveArea> {
        self.areas
            .iter()
            .filter(|area| match self.parent {
                Some(parent) => area.parent_id == parent,
                None => area.id == 0,
            })
            .collect()
    }
}

impl App {
    pub(in crate::app) fn open_live_areas(&mut self) {
        self.direction_hold = None;
        self.live_list.parent =
            (self.live_list.filter.id > 0).then_some(self.live_list.filter.parent_id);
        self.ui.global::<crate::LiveAreas>().set_open(true);
        if self.live_list.areas.is_empty() {
            self.load_live_areas();
        } else {
            self.sync_live_areas();
        }
    }

    fn load_live_areas(&self) {
        let panel = self.ui.global::<crate::LiveAreas>();
        if panel.get_loading() {
            return;
        }
        panel.set_loading(true);
        panel.set_error("".into());
        let tx = self.tx.clone();
        thread::spawn(move || {
            crate::cache::set_force(true);
            let _ = tx.send(Msg::LiveAreas(crate::api::live::areas()));
        });
    }

    pub(in crate::app) fn live_areas_loaded(
        &mut self,
        result: Result<Vec<model::LiveArea>, String>,
    ) {
        self.ui.global::<crate::LiveAreas>().set_loading(false);
        match result {
            Ok(areas) => {
                self.live_list.areas = areas;
                self.sync_live_areas();
            }
            Err(error) => {
                crate::diagnostics::write(format_args!("[直播分区] {error}"));
                self.ui
                    .global::<crate::LiveAreas>()
                    .set_error("分区加载失败，按刷新重试".into());
            }
        }
    }

    fn sync_live_areas(&self) {
        let options = self.live_list.options();
        let panel = self.ui.global::<crate::LiveAreas>();
        let title = self.live_list.parent.and_then(|parent| {
            self.live_list
                .areas
                .iter()
                .find(|area| area.parent_id == parent && area.id == 0)
        });
        panel.set_title(title.map_or_else(
            || "选择直播分区".into(),
            |area| format!("选择分区 · {}", area.name).into(),
        ));
        panel.set_selected(
            options
                .iter()
                .position(|area| {
                    area.parent_id == self.live_list.filter.parent_id
                        && area.id == self.live_list.filter.id
                })
                .unwrap_or(0) as i32,
        );
        panel.set_rows(ModelRc::new(VecModel::from(
            options
                .iter()
                .map(|area| {
                    if self.live_list.parent.is_some() && area.id == 0 {
                        "全部".into()
                    } else {
                        area.name.as_str().into()
                    }
                })
                .collect::<Vec<slint::SharedString>>(),
        )));
    }

    pub(in crate::app) fn live_area_key(&mut self, key: &str) {
        let panel = self.ui.global::<crate::LiveAreas>();
        if matches!(key, "ESC" | "BACKSPACE") {
            if self.live_list.parent.take().is_some() {
                self.sync_live_areas();
            } else {
                panel.set_open(false);
            }
            self.direction_hold = None;
            self.return_hold = None;
            return;
        }
        if key == "r" {
            self.load_live_areas();
            return;
        }
        if panel.get_loading() || !panel.get_error().is_empty() {
            return;
        }
        let options = self.live_list.options();
        let last = (options.len() as i32 - 1).max(0);
        let selected = panel.get_selected();
        let step = match key {
            "UP" => -3,
            "DOWN" => 3,
            "LEFT" => -1,
            "RIGHT" => 1,
            _ => 0,
        };
        if step != 0 {
            panel.set_selected((selected + step).clamp(0, last));
            return;
        }
        let index = if matches!(key, "ENTER" | "SPACE") {
            Some(selected as usize)
        } else {
            key.strip_prefix("AREA_SELECT:")
                .and_then(|index| index.parse().ok())
        };
        let Some(area) = index
            .and_then(|index| options.get(index))
            .map(|area| (*area).clone())
        else {
            return;
        };
        if self.live_list.parent.is_none() && area.parent_id > 0 {
            self.live_list.parent = Some(area.parent_id);
            self.sync_live_areas();
            return;
        }
        panel.set_open(false);
        self.direction_hold = None;
        if area.parent_id == self.live_list.filter.parent_id && area.id == self.live_list.filter.id
        {
            return;
        }
        self.live_list.filter = area;
        let source = HomeTab::Live.index();
        // 切换分区时清空旧列表，刷新与翻页共用新的请求编号
        self.lists[source].clear();
        self.covers[source].clear();
        self.loaded[source] = false;
        self.pages[source] = 1;
        self.first_page_sizes[source] = 0;
        self.has_more[source] = true;
        self.sel = 0;
        self.list_scroll = 0.0;
        self.grid_scroll = 0.0;
        self.home_dirty = true;
        self.loading = true;
        self.status = "加载中…".into();
        self.status_detail.clear();
        self.load_home_list(source, false);
    }

    pub(in crate::app) fn live_list_loaded(
        &mut self,
        request: u64,
        page: u32,
        result: Result<model::VideoPage, String>,
    ) {
        let source = HomeTab::Live.index();
        if request != self.live_list.request
            || (page > 1 && page as usize != self.pages[source] + 1)
        {
            return;
        }
        match result {
            Ok(data) => {
                if page == 1 {
                    let update_more = self.pages[source] <= 1 && !self.loading_more[source];
                    self.list_loaded(source, data.items);
                    if update_more {
                        self.has_more[source] = data.has_more;
                    }
                } else {
                    let previous = self.lists[source].len();
                    self.list_more(source, data.items);
                    self.has_more[source] = data.has_more && self.lists[source].len() > previous;
                }
            }
            Err(error) => self.list_error(source, error),
        }
    }
}
