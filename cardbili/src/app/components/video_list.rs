use slint::ComponentHandle as _;
use super::super::*;

pub(in crate::app) fn video_columns(view: VideoLayout) -> usize {
    match view {
        VideoLayout::List => 1,
        VideoLayout::TwoColumns => 2,
        VideoLayout::ThreeColumns => 3,
    }
}

pub(in crate::app) fn video_row(item: &model::Item) -> VideoRow {
    VideoRow {
        title: item.title.as_str().into(),
        author: item.meta.as_str().into(),
        duration: item.duration.as_str().into(),
        views: item.views.as_deref().unwrap_or_default().into(),
        likes: item.likes.as_deref().unwrap_or_default().into(),
        danmaku: item.danmaku.as_deref().unwrap_or_default().into(),
        progress: progress_text(item),
        watched: model::watch_time(item.view_at).into(),
    }
}

fn progress_text(item: &model::Item) -> slint::SharedString {
    if item.progress < 0 {
        return "已看完".into();
    }
    if item.progress == 0 {
        return Default::default();
    }
    if item.duration_secs > 0.0 && item.progress as f64 >= item.duration_secs - 1.0 {
        return "已看完".into();
    }
    if item.duration_secs > 0.0 {
        return format!(
            "{} / {}",
            clock_text(item.progress),
            clock_text(item.duration_secs as i64)
        )
        .into();
    }
    clock_text(item.progress).into()
}

fn clock_text(sec: i64) -> String {
    let sec = sec.max(0);
    if sec >= 3600 {
        format!("{}:{:02}:{:02}", sec / 3600, (sec % 3600) / 60, sec % 60)
    } else {
        format!("{:02}:{:02}", sec / 60, sec % 60)
    }
}

pub(in crate::app) fn compact_card_count(value: &str) -> slint::SharedString {
    if let Some((whole, fraction)) = value.split_once('.') {
        let unit = fraction.chars().last();
        if whole.len() >= 3 && matches!(unit, Some('万' | '亿')) {
            return format!("{}{}", whole, unit.unwrap()).into();
        }
    }
    value.into()
}

pub(in crate::app) fn sync_list(ui: &AppWindow, items: &[model::Item], group_history: bool, first_row: usize) {
    let mut rows = Vec::with_capacity(items.len());
    let mut tops = Vec::with_capacity(items.len());
    let mut day_starts = Vec::with_capacity(items.len());
    let mut headers = Vec::new();
    let mut extra = 0.0f32;
    let mut last_day = None;
    for (index, item) in items.iter().enumerate() {
        let mut day_start = false;
        if group_history && item.view_at > 0 {
            let day = (item.view_at + 8 * 3600).div_euclid(86400);
            if last_day != Some(day) {
                headers.push(crate::HistoryHeader {
                    title: model::history_day_label(item.view_at).into(),
                    top: index as f32 * 30.0 + extra,
                });
                extra += 24.0;
                last_day = Some(day);
                day_start = true;
            }
        }
        rows.push(video_row(item));
        tops.push(index as f32 * 30.0 + extra);
        day_starts.push(day_start);
    }
    let scroll_top = tops
        .get(first_row)
        .map(|top| if day_starts.get(first_row) == Some(&true) { top - 24.0 } else { *top })
        .unwrap_or(0.0);
    let list_model = ui.global::<crate::ListModel>();
    list_model.set_rows(ModelRc::new(VecModel::from(rows)));
    list_model.set_headers(ModelRc::new(VecModel::from(headers)));
    list_model.set_row_tops(ModelRc::new(VecModel::from(tops)));
    list_model.set_scroll_top(scroll_top);
}

pub(in crate::app) fn list_source(tab: HomeTab, mine_detail: i32) -> Option<usize> {
    match tab {
        HomeTab::Recommended | HomeTab::Popular | HomeTab::Dynamic => Some(tab.index()),
        HomeTab::Mine if mine_detail >= 0 => Some(mine_detail as usize),
        _ => None,
    }
}

// 首屏刷新保留后续页、已解码封面和当前视频的选中位置
pub(in crate::app) fn refresh_first_page(
    items: &mut Vec<model::Item>,
    covers: &mut Vec<Option<slint::Image>>,
    first_page_size: &mut usize,
    mut fresh: Vec<model::Item>,
    selection: i32,
) -> i32 {
    let selected = items.get(selection as usize).map(|item| item.bvid.clone());
    let images: HashMap<_, _> = items
        .iter()
        .zip(covers.iter())
        .map(|(item, image)| (item.bvid.clone(), image.clone()))
        .collect();
    let mut known: HashSet<_> = fresh.iter().map(|item| item.bvid.clone()).collect();
    let tail = items
        .drain((*first_page_size).min(items.len())..)
        .filter(|item| known.insert(item.bvid.clone()))
        .collect::<Vec<_>>();
    *first_page_size = fresh.len();
    fresh.extend(tail);
    *items = fresh;
    *covers = items
        .iter()
        .map(|item| images.get(&item.bvid).cloned().flatten())
        .collect();
    selected
        .and_then(|bvid| items.iter().position(|item| item.bvid == bvid))
        .map(|index| index as i32)
        .unwrap_or(selection.clamp(0, (items.len() as i32 - 1).max(0)))
}

impl App {
    pub(in crate::app) fn video_list_key(&mut self, key: &str, len: usize) -> bool {
        if key == "v" {
            self.view = self.view.cycle(1);
            self.settings_changed = true;
            return true;
        }
        let layout = if self.tab == HomeTab::Mine && self.mine_detail == 3 {
            VideoLayout::List
        } else {
            self.view
        };
        let columns = video_columns(layout) as i32;
        let step = match key {
            "UP" => Some(-columns),
            "DOWN" => Some(columns),
            "LEFT" => Some(-1),
            "RIGHT" => Some(1),
            _ => None,
        };
        if let Some(step) = step {
            let next = (self.sel + step).max(0);
            if next < len as i32 {
                self.sel = next;
            }
            return true;
        }
        false
    }
}
