use slint::ComponentHandle as _;
use super::*;

#[derive(Default)]
pub(super) struct FollowingState {
    pub items: Vec<model::Following>,
    pub avatars: HashMap<String, slint::Image>,
    pub page: u32,
    pub pending_page: u32,
    pub first_page_size: usize,
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
            self.following.pending_page = self.following.page + 1;
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
        if self.page != Page::Following || request != self.following_request {
            return;
        }
        if page == self.following.pending_page {
            self.following.loading = false;
        }
        match result {
            Ok(result) => {
                tasks::load_following_avatars(result.items.clone(), request, self.tx.clone());
                if page == 1 {
                    let tail = self.following.items.split_off(
                        self.following
                            .first_page_size
                            .min(self.following.items.len()),
                    );
                    self.following.first_page_size = result.items.len();
                    self.following.items = result.items;
                    self.following.items.extend(tail);
                } else {
                    self.following.items.extend(result.items);
                }
                let mut known = HashSet::new();
                self.following
                    .items
                    .retain(|user| known.insert(user.owner.mid.clone()));
                self.following.selection = self
                    .following
                    .selection
                    .min(self.following.items.len().saturating_sub(1));
                if page >= self.following.page {
                    self.following.page = page;
                    self.following.has_more = result.has_more;
                }
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
        if self.page != Page::Following {
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
        let g_following = self.ui.global::<crate::Following>();
        if self.following.dirty {
            self.following.first = self
                .following
                .first
                .min(selected)
                .max(selected.saturating_sub(3));
            g_following.set_rows(ModelRc::new(VecModel::from(
                self.following
                    .items
                    .iter()
                    .skip(self.following.first)
                    .take(4)
                    .map(|user| crate::FollowingRow {
                        avatar: self
                            .following
                            .avatars
                            .get(&user.owner.mid)
                            .cloned()
                            .unwrap_or_default(),
                        has_avatar: self.following.avatars.contains_key(&user.owner.mid),
                        user: user.owner.name.as_str().into(),
                        text: if user.signature.is_empty() {
                            format!("UID {}", user.owner.mid).into()
                        } else {
                            user.signature.as_str().into()
                        },
                    })
                    .collect::<Vec<_>>(),
            )));
            g_following.set_selected((selected - self.following.first) as i32);
            self.following.dirty = false;
        }
        g_following.set_loading(self.following.loading);
        g_following.set_error(self.following.error.as_str().into());
    }
}
