use super::*;

/// 根据可视高度与滚动范围更新详情页滚动条
pub(in crate::app) fn sync_thumb(ui: &AppWindow, scroll: f64, maxv: f64) {
    let vh = ui.get_detail_viewport_h() as f64;
    if maxv <= 0.5 {
        ui.set_detail_thumb_h(0.0);
        ui.set_detail_thumb_y(0.0);
        return;
    }
    let h = (vh * (vh / (maxv + vh))).max(14.0);
    let y = (scroll / maxv) * (vh - h);
    ui.set_detail_thumb_h(h as f32);
    ui.set_detail_thumb_y(y as f32);
}

pub(in crate::app) fn show_detail(ui: &AppWindow, display: &DetailDisplay) {
    let detail = &display.data;
    ui.set_detail_loading(false);
    ui.set_detail_error("".into());
    ui.set_detail_title(detail.title.as_str().into());
    ui.set_detail_owner(
        if detail.owner.name.is_empty() {
            "未知用户"
        } else {
            &detail.owner.name
        }
        .into(),
    );
    ui.set_detail_desc(
        if detail.desc.trim().is_empty() {
            "（无简介）"
        } else {
            detail.desc.trim()
        }
        .into(),
    );
    let metrics = [
        model::mmss(detail.duration),
        model::fmt_num(detail.views),
        model::fmt_num(detail.likes),
        model::fmt_num(detail.coins),
        model::fmt_num(detail.favorites),
        model::fmt_num(detail.danmaku),
    ];
    ui.set_detail_metrics(ModelRc::new(VecModel::from(
        metrics
            .into_iter()
            .map(slint::SharedString::from)
            .collect::<Vec<_>>(),
    )));
    ui.set_has_detail_cover(display.cover.is_some());
    if let Some(cover) = &display.cover {
        ui.set_detail_cover(cover.clone());
    }
}

pub(in crate::app) fn show_detail_loading(ui: &AppWindow, title: &str) {
    ui.set_detail_loading(true);
    ui.set_detail_error("".into());
    ui.set_detail_title(title.into());
    ui.set_detail_metrics(ModelRc::new(VecModel::from(
        Vec::<slint::SharedString>::new(),
    )));
    ui.set_detail_desc("".into());
    ui.set_detail_owner("".into());
    ui.set_has_detail_cover(false);
    ui.set_detail_scroll(0.0);
    ui.set_detail_thumb_h(0.0);
}
