use super::*;
use slint::ComponentHandle as _;

#[derive(Clone, Default)]
pub(super) struct Comments {
    pub aid: i64,
    pub items: Vec<model::Comment>,
    pub page: u32,
    pub loading: bool,
    pub has_more: bool,
    pub error: String,
    pub scroll: f64,
    pub dirty: bool,
    pub requested_emotes: std::collections::HashSet<String>,
    rows: Vec<crate::CommentRow>,
    visible: std::ops::Range<usize>,
    width: f32,
    layout_revision: i32,
    list_offset: f32,
}

impl App {
    pub(super) fn open_comments(&mut self, aid: i64) {
        self.page = Page::Comments;
        self.start_comments(aid);
    }

    pub(super) fn start_comments(&mut self, aid: i64) {
        self.comment_page = Comments {
            aid,
            has_more: true,
            dirty: true,
            ..Default::default()
        };
        self.load_comments();
    }

    pub(super) fn load_comments(&mut self) {
        if self.comment_page.loading || !self.comment_page.has_more {
            return;
        }
        self.comment_page.loading = true;
        self.comment_page.error.clear();
        tasks::playback::comments(
            self.comment_page.aid,
            self.comment_page.page + 1,
            self.cookies.clone(),
            self.comments_request,
            self.tx.clone(),
        );
    }

    pub(super) fn comments_loaded(
        &mut self,
        request: u64,
        page: u32,
        result: Result<model::CommentPage, String>,
    ) {
        if !matches!(self.page, Page::Comments | Page::Detail) || request != self.comments_request {
            return;
        }
        self.comment_page.loading = false;
        match result {
            Ok(result) => {
                self.comment_page.has_more = result.has_more && !result.items.is_empty();
                let mut pending = Vec::new();
                for item in &result.items {
                    for emote in &item.emotes {
                        if !crate::emoji::has_emote(&emote.name)
                            && self
                                .comment_page
                                .requested_emotes
                                .insert(emote.name.clone())
                        {
                            pending.push(emote.clone());
                        }
                    }
                }
                if !pending.is_empty() {
                    tasks::playback::comment_emotes(pending, self.tx.clone());
                }
                self.comment_page.items.extend(result.items);
                self.comment_page.page = page;
            }
            Err(error) => self.comment_page.error = error,
        }
    }

    pub(super) fn comments_key(&mut self, key: &str) {
        if self.scroll_key(key) {
            return;
        }
        match key {
            "r" => self.load_comments(),
            "ESC" | "BACKSPACE" => self.go_back = true,
            _ => {}
        }
    }

    pub(super) fn scroll_comments(&mut self, step: f64) {
        let limit = (self.ui.global::<crate::Comments>().get_content_h() as f64
            - self.ui.global::<crate::Comments>().get_viewport_h() as f64)
            .max(0.0);
        self.comment_page.scroll = (self.comment_page.scroll + step).clamp(0.0, limit);
        if step > 0.0
            && self.comment_page.scroll >= limit - 60.0
            && self.comment_page.has_more
            && self.comment_page.error.is_empty()
        {
            self.load_comments();
        }
    }

    pub(super) fn sync_comments(&mut self) {
        let comments = self.ui.global::<crate::Comments>();
        if !matches!(self.page, Page::Comments | Page::Detail) {
            return;
        }
        comments.set_scroll(self.comment_page.scroll as f32);
        let page = &mut self.comment_page;
        let scroll = if self.page == Page::Detail {
            self.detail_scroll
        } else {
            page.scroll
        };
        if !page.dirty {
            page.list_offset = comments.get_list_offset();
        }
        let anchor = page
            .rows
            .partition_point(|row| row.y <= scroll as f32 - page.list_offset)
            .saturating_sub(1);
        let anchor_top = page.rows.get(anchor).map_or(0.0, |row| row.y);
        let width = comments.get_list_width();
        let revision = self.ui.global::<crate::EmojiFont>().get_layout_revision();
        let spacing = comments.get_spacing();
        let mut refresh = page.dirty;

        if page.width != width || page.layout_revision != revision {
            let mut top = 0.0;
            for (index, (row, item)) in page.rows.iter_mut().zip(&page.items).enumerate() {
                let height = if page.width != width
                    || crate::emoji::layout_changed(&item.text, page.layout_revision)
                {
                    comments.invoke_row_height(row.text.clone())
                } else {
                    row.height
                };
                if row.y != top || row.height != height {
                    row.y = top;
                    row.height = height;
                    refresh |= page.visible.contains(&index);
                }
                top += height + spacing;
            }
            page.width = width;
            page.layout_revision = revision;
        }

        let mut top = page
            .rows
            .last()
            .map_or(0.0, |row| row.y + row.height + spacing);
        for item in &page.items[page.rows.len()..] {
            let text: slint::SharedString = item.text.as_str().into();
            let height = comments.invoke_row_height(text.clone());
            page.rows.push(crate::CommentRow {
                user: item.user.as_str().into(),
                text,
                time: model::watch_time(item.created_at).into(),
                likes: model::fmt_num(item.likes).into(),
                y: top,
                height,
            });
            top += height + spacing;
        }
        comments.set_count(page.rows.len() as i32);
        comments.set_rows_height((top - spacing).max(0.0));
        comments.set_finished(page.page > 0 && !page.has_more);
        comments.set_loading(page.loading);
        comments.set_error(page.error.as_str().into());

        // 行高变化时保持首个可见评论的位置，并限制在有效滚动范围内
        let shift = f64::from(page.rows.get(anchor).map_or(0.0, |row| row.y) - anchor_top);
        let limit = f64::from(
            (page.list_offset + comments.get_content_h() - comments.get_viewport_h()).max(0.0),
        );
        if self.page == Page::Detail {
            let detail = self.ui.global::<crate::Detail>();
            if !detail.get_loading() {
                self.detail_maxv = limit;
                self.detail_scroll = if std::env::var("BILI_DSEND").is_ok() {
                    limit
                } else {
                    (self.detail_scroll + shift).clamp(0.0, limit)
                };
                detail.set_scroll(self.detail_scroll as f32);
                sync_thumb(&self.ui, self.detail_scroll, limit);
            }
        } else {
            page.scroll = (page.scroll + shift).clamp(0.0, limit);
            comments.set_scroll(page.scroll as f32);
        }

        let scroll = if self.page == Page::Detail {
            self.detail_scroll
        } else {
            page.scroll
        } as f32;
        let top = scroll - page.list_offset;
        let first = page
            .rows
            .partition_point(|row| row.y + row.height < top - 64.0);
        let end = page
            .rows
            .partition_point(|row| row.y < top + comments.get_viewport_h() + 64.0);
        let visible = first..end;
        if refresh || page.visible != visible {
            comments.set_rows(ModelRc::new(VecModel::from(
                page.rows[visible.clone()].to_vec(),
            )));
            page.visible = visible;
        }
        page.dirty = false;
    }
}
