use super::super::*;

pub(in crate::app) fn video_columns(view: i32) -> usize {
    match view {
        0 => 1,
        2 => 2,
        _ => 3,
    }
}

pub(in crate::app) fn video_row(
    item: &model::Item,
    stats: &HashMap<String, model::VideoStats>,
) -> VideoRow {
    let cached = stats.get(&item.bvid);
    VideoRow {
        title: item.title.as_str().into(),
        author: item.meta.as_str().into(),
        duration: item.duration.as_str().into(),
        views: cached
            .map(|stats| stats.views.as_str())
            .or(item.views.as_deref())
            .unwrap_or("—")
            .into(),
        likes: cached
            .map(|stats| stats.likes.as_str())
            .or(item.likes.as_deref())
            .unwrap_or("—")
            .into(),
        danmaku: cached
            .map(|stats| stats.danmaku.as_str())
            .or(item.danmaku.as_deref())
            .unwrap_or("—")
            .into(),
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

pub(in crate::app) fn sync_list(
    ui: &AppWindow,
    items: &[model::Item],
    stats: &HashMap<String, model::VideoStats>,
) {
    let rows = items
        .iter()
        .map(|item| video_row(item, stats))
        .collect::<Vec<_>>();
    ui.set_list_rows(ModelRc::new(VecModel::from(rows)));
}

pub(in crate::app) fn list_source(tab: usize, mine_detail: i32) -> Option<usize> {
    match tab {
        0..=2 => Some(tab),
        3 if mine_detail >= 0 => Some(mine_detail as usize),
        _ => None,
    }
}
