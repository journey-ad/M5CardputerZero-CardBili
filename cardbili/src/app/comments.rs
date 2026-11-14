use super::*;

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
                self.comment_page.items.extend(result.items);
                self.comment_page.page = page;
                self.comment_page.dirty = true;
            }
            Err(error) => self.comment_page.error = error,
        }
    }

    pub(super) fn comments_key(&mut self, key: &str) {
        match key {
            "r" => self.load_comments(),
            "ESC" | "BACKSPACE" => self.go_back = true,
            _ => {}
        }
    }

    pub(super) fn scroll_comments(&mut self, step: f64) {
        let limit = (self.ui.get_comment_content_h() as f64
            - self.ui.get_comment_viewport_h() as f64)
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
        if !matches!(self.page, Page::Comments | Page::Detail) {
            return;
        }
        if self.comment_page.dirty {
            self.ui.set_comment_rows(ModelRc::new(VecModel::from(
                self.comment_page
                    .items
                    .iter()
                    .map(|item| Cmt {
                        user: item.user.as_str().into(),
                        text: item.text.as_str().into(),
                    })
                    .collect::<Vec<_>>(),
            )));
            self.comment_page.dirty = false;
        }
        self.ui
            .set_comment_finished(self.comment_page.page > 0 && !self.comment_page.has_more);
        self.ui.set_comment_loading(self.comment_page.loading);
        self.ui
            .set_comment_error(self.comment_page.error.as_str().into());
        self.ui.set_comment_scroll(self.comment_page.scroll as f32);
    }
}
