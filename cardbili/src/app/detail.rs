use super::*;
use slint::ComponentHandle as _;

#[derive(Clone)]
pub(super) struct DetailDisplay {
    pub(super) data: model::Detail,
    pub(super) cover: Option<slint::Image>,
}

/// 根据可视高度与滚动范围更新详情页滚动条
pub(in crate::app) fn sync_thumb(ui: &AppWindow, scroll: f64, maxv: f64) {
    let detail = ui.global::<crate::Detail>();
    let vh = detail.get_viewport_h() as f64;
    if maxv <= 0.5 {
        detail.set_thumb_h(0.0);
        detail.set_thumb_y(0.0);
        return;
    }
    let h = (vh * (vh / (maxv + vh))).max(14.0);
    let y = (scroll / maxv) * (vh - h);
    detail.set_thumb_h(h as f32);
    detail.set_thumb_y(y as f32);
}

pub(in crate::app) fn show_detail(ui: &AppWindow, display: &DetailDisplay) {
    let g_detail = ui.global::<crate::Detail>();
    let detail = &display.data;
    g_detail.set_loading(false);
    g_detail.set_error("".into());
    g_detail.set_title(detail.title.as_str().into());
    g_detail.set_owner(
        if detail.owner.name.is_empty() {
            "未知用户"
        } else {
            &detail.owner.name
        }
        .into(),
    );
    g_detail.set_desc(
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
    g_detail.set_metrics(ModelRc::new(VecModel::from(
        metrics
            .into_iter()
            .map(slint::SharedString::from)
            .collect::<Vec<_>>(),
    )));
    g_detail.set_has_cover(display.cover.is_some());
    if let Some(cover) = &display.cover {
        g_detail.set_cover(cover.clone());
    }
}

pub(in crate::app) fn show_detail_loading(ui: &AppWindow, title: &str) {
    let detail = ui.global::<crate::Detail>();
    detail.set_loading(true);
    detail.set_error("".into());
    detail.set_title(title.into());
    detail.set_metrics(ModelRc::new(VecModel::from(
        Vec::<slint::SharedString>::new(),
    )));
    detail.set_desc("".into());
    detail.set_owner("".into());
    detail.set_has_cover(false);
    detail.set_scroll(0.0);
    detail.set_thumb_h(0.0);
}

impl App {
    pub(super) fn detail_loaded(
        &mut self,
        request: u64,
        data: model::Detail,
        (w, h, raw): crate::cache::Pixels,
    ) {
        if self.page != Page::Detail || request != self.detail_request {
            return;
        }
        let display = DetailDisplay {
            data,
            cover: if raw.is_empty() {
                None
            } else {
                Some(util::raw_to_image(w, h, &raw))
            },
        };
        if self.comment_page.aid != display.data.aid {
            self.start_comments(display.data.aid);
        }
        self.setup_sections(
            self.detail_bvid.clone(),
            display.data.cid,
            display.data.parts.clone(),
        );
        self.marq_t = 0.0;
        show_detail(&self.ui, &display);
        self.detail_display = Some(display);
    }

    pub(super) fn detail_key(&mut self, k: &str) {
        if k == "p" {
            self.sections.open = true;
            self.sections.dirty = true;
            return;
        }
        if self.scroll_key(k) {
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
