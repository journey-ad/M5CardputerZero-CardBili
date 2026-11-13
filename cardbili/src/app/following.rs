use super::*;

#[derive(Default)]
pub(super) struct FollowingState {
    pub items: Vec<model::Following>,
    pub page: u32,
    pub has_more: bool,
    pub loading: bool,
    pub error: String,
    pub selection: usize,
    pub first: usize,
    pub dirty: bool,
}

impl App {
    pub(super) fn load_following(&mut self) {
        if self.following.loading {
            return;
        }
        if let Some(cookies) = &self.cookies {
            self.following.loading = true;
            self.following.error.clear();
            tasks::load_following(
                self.mid.clone(),
                self.following.page + 1,
                cookies.clone(),
                self.following_request,
                self.tx.clone(),
            );
        }
    }

    pub(super) fn following_loaded(
        &mut self,
        request: u64,
        page: u32,
        result: Result<model::FollowingPage, String>,
    ) {
        if self.page != 7 || request != self.following_request {
            return;
        }
        self.following.loading = false;
        match result {
            Ok(result) => {
                let mut known: HashSet<String> = self
                    .following
                    .items
                    .iter()
                    .map(|user| user.owner.mid.clone())
                    .collect();
                self.following.items.extend(
                    result
                        .items
                        .into_iter()
                        .filter(|user| known.insert(user.owner.mid.clone())),
                );
                self.following.page = page;
                self.following.has_more = result.has_more;
                self.following.dirty = true;
            }
            Err(error) => self.following.error = error,
        }
    }

    pub(super) fn following_key(&mut self, key: &str) {
        let previous = self.following.selection;
        match key {
            "UP" | "f" | "w" => self.following.selection = previous.saturating_sub(1),
            "DOWN" | "x" | "s" => {
                self.following.selection =
                    (previous + 1).min(self.following.items.len().saturating_sub(1))
            }
            "LEFT" | "z" => self.following.selection = previous.saturating_sub(4),
            "RIGHT" | "c" => {
                self.following.selection =
                    (previous + 4).min(self.following.items.len().saturating_sub(1))
            }
            "ENTER" | "SPACE" | "u" => {
                self.requested_uploader = self
                    .following
                    .items
                    .get(previous)
                    .map(|user| user.owner.clone());
            }
            "r" => self.load_following(),
            "ESC" | "BACKSPACE" => self.go_back = true,
            _ => {}
        }
        self.following.dirty |= previous != self.following.selection;
    }

    pub(super) fn sync_following(&mut self) {
        if self.page != 7 {
            return;
        }
        let selected = self.following.selection;
        if self.following.has_more
            && !self.following.loading
            && self.following.error.is_empty()
            && selected + 4 >= self.following.items.len()
        {
            self.load_following();
        }
        if self.following.dirty {
            self.following.first = self
                .following
                .first
                .min(selected)
                .max(selected.saturating_sub(3));
            self.ui.set_following_rows(ModelRc::new(VecModel::from(
                self.following
                    .items
                    .iter()
                    .skip(self.following.first)
                    .take(4)
                    .map(|user| Cmt {
                        user: user.owner.name.as_str().into(),
                        text: if user.signature.is_empty() {
                            format!("UID {}", user.owner.mid).into()
                        } else {
                            user.signature.as_str().into()
                        },
                    })
                    .collect::<Vec<_>>(),
            )));
            self.ui
                .set_following_selected((selected - self.following.first) as i32);
            self.following.dirty = false;
        }
        self.ui.set_following_loading(self.following.loading);
        self.ui
            .set_following_error(self.following.error.as_str().into());
    }
}
