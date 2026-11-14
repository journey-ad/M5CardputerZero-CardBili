use super::super::*;
use super::video_list::video_row;

pub(in crate::app) struct HistoryRows {
    pub rows: Vec<VideoRow>,
    pub headers: Vec<crate::HistoryHeader>,
    pub tops: Vec<f32>,
    pub offsets: Vec<f32>,
}

pub(in crate::app) fn build_history_rows(items: &[model::Item]) -> HistoryRows {
    let mut rows = Vec::with_capacity(items.len());
    let mut headers = Vec::new();
    let mut tops = Vec::with_capacity(items.len());
    let mut offsets = Vec::with_capacity(items.len());
    let mut extra = 0.0f32;
    let mut last_day = None;
    for (index, item) in items.iter().enumerate() {
        let mut day_start = false;
        if item.view_at > 0 {
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
        let mut row = video_row(item);
        row.progress = progress_text(item);
        row.watched = model::watch_time(item.view_at).into();
        rows.push(row);
        let top = index as f32 * 30.0 + extra;
        tops.push(top);
        offsets.push(if day_start { top - 24.0 } else { top });
    }
    HistoryRows {
        rows,
        headers,
        tops,
        offsets,
    }
}

pub(in crate::app) fn progress_text(item: &model::Item) -> slint::SharedString {
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
