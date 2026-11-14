use slint::ComponentHandle as _;
use super::*;

/// 根据可视高度与滚动范围更新详情页滚动条
pub(in crate::app) fn sync_thumb(ui: &AppWindow, scroll: f64, maxv: f64) {
    let vh = ui.get_detail_viewport_h() as f64;
    if maxv <= 0.5 {
        ui.global::<crate::Detail>().set_thumb_h(0.0);
        ui.global::<crate::Detail>().set_thumb_y(0.0);
        return;
    }
    let h = (vh * (vh / (maxv + vh))).max(14.0);
    let y = (scroll / maxv) * (vh - h);
    ui.global::<crate::Detail>().set_thumb_h(h as f32);
    ui.global::<crate::Detail>().set_thumb_y(y as f32);
}

pub(in crate::app) fn show_detail(ui: &AppWindow, display: &DetailDisplay) {
    let detail = &display.data;
    ui.global::<crate::Detail>().set_loading(false);
    ui.global::<crate::Detail>().set_error("".into());
    ui.global::<crate::Detail>().set_title(detail.title.as_str().into());
    ui.global::<crate::Detail>().set_owner(
        if detail.owner.name.is_empty() {
            "未知用户"
        } else {
            &detail.owner.name
        }
        .into(),
    );
    ui.global::<crate::Detail>().set_desc(
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
    ui.global::<crate::Detail>().set_metrics(ModelRc::new(VecModel::from(
        metrics
            .into_iter()
            .map(slint::SharedString::from)
            .collect::<Vec<_>>(),
    )));
    ui.global::<crate::Detail>().set_has_cover(display.cover.is_some());
    if let Some(cover) = &display.cover {
        ui.global::<crate::Detail>().set_cover(cover.clone());
    }
}

pub(in crate::app) fn show_detail_loading(ui: &AppWindow, title: &str) {
    ui.global::<crate::Detail>().set_loading(true);
    ui.global::<crate::Detail>().set_error("".into());
    ui.global::<crate::Detail>().set_title(title.into());
    ui.global::<crate::Detail>().set_metrics(ModelRc::new(VecModel::from(
        Vec::<slint::SharedString>::new(),
    )));
    ui.global::<crate::Detail>().set_desc("".into());
    ui.global::<crate::Detail>().set_owner("".into());
    ui.global::<crate::Detail>().set_has_cover(false);
    ui.global::<crate::Detail>().set_scroll(0.0);
    ui.global::<crate::Detail>().set_thumb_h(0.0);
}

impl App {
    pub(super) fn detail_key(&mut self, k: &str, repeat: bool) {
        if k == "p" {
            self.sections.open = true;
            self.sections.dirty = true;
            return;
        }
        if self.scroll_key(k, repeat) {
            return;
        }
        match k {
            "ENTER" | "SPACE" => {
                if !self.detail_bvid.is_empty() {
                    self.status = "正在加载视频…".into();
                    self.status_detail.clear();
                    self.requested_play = Some(self.detail_bvid.clone());
                }
            }
            "u" => {
                self.requested_uploader = self
                    .detail_display
                    .as_ref()
                    .map(|display| display.data.owner.clone())
                    .filter(|owner| !owner.mid.is_empty());
            }
            "RELATED" => {
                self.requested_related = Some(self.detail_bvid.clone());
            }
            "r" => {
                if !self.comment_page.error.is_empty() {
                    self.load_comments();
                } else if !self.detail_bvid.is_empty() {
                    self.requested_related = Some(self.detail_bvid.clone());
                }
            }
            "ESC" | "BACKSPACE" => {
                self.go_back = true;
            }
            _ => {}
        }
    }
}
