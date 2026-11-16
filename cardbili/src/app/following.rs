use super::*;
use crate::api;
use slint::ComponentHandle as _;

// 关注页列表项：分组标题行或具体用户
pub(super) enum FollowingEntry {
    Header { name: String, count: u64 },
    User(model::Following),
}

#[derive(Default, Clone)]
pub(super) struct FollowingGroup {
    pub tagid: i64,
    pub name: String,
    pub count: u64,
    pub page: u32,
}

#[derive(Default)]
pub(super) struct FollowingState {
    pub items: Vec<FollowingEntry>,
    pub avatars: HashMap<String, slint::Image>,
    pub groups: Vec<FollowingGroup>,
    pub group_index: usize,
    pub tags_loaded: bool,
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
        let Some(cookies) = self.cookies.clone() else {
            return;
        };
        self.following.loading = true;
        self.following.error.clear();
        if self.following.tags_loaded {
            self.load_next_group(cookies);
        } else {
            tasks::load_following_tags(cookies, self.following_request, self.tx.clone());
        }
    }

    // 拉当前分组的下一页；分组标题行在数据返回时补上
    fn load_next_group(&mut self, cookies: String) {
        let Some(group) = self.following.groups.get(self.following.group_index) else {
            self.following.loading = false;
            self.following.has_more = false;
            return;
        };
        let (tagid, page) = (group.tagid, group.page + 1);
        let (mid, req, tx) = (self.mid.clone(), self.following_request, self.tx.clone());
        if tagid == api::ALL_FOLLOWING_TAGID {
            tasks::load_all_followings(mid, page, cookies, req, tx);
        } else {
            tasks::load_following_group(tagid, page, cookies, req, tx);
        }
    }

    pub(super) fn following_tags_loaded(
        &mut self,
        request: u64,
        result: Result<Vec<api::FollowingTag>, String>,
    ) {
        if self.page != Page::Following || request != self.following_request {
            return;
        }
        self.following.loading = false;
        let tags = match result {
            Ok(tags) => tags,
            Err(error) => {
                self.following.error = error;
                self.following.dirty = true;
                return;
            }
        };
        self.following.tags_loaded = true;
        // 分组之间互斥，各分组 count 之和即全部关注人数
        let total = tags.iter().map(|tag| tag.count).sum::<u64>();
        self.following.groups.clear();
        // 特别关注置顶，其次是全部关注，最后是其余分组
        if let Some(special) = tags.iter().find(|tag| tag.tagid == -10 && tag.count > 0) {
            self.following.groups.push(FollowingGroup {
                tagid: -10,
                name: special.name.clone(),
                count: special.count,
                page: 0,
            });
        }
        if total > 0 {
            self.following.groups.push(FollowingGroup {
                tagid: api::ALL_FOLLOWING_TAGID,
                name: "全部关注".into(),
                count: total,
                page: 0,
            });
        }
        self.following
            .groups
            .extend(
                tags.into_iter()
                    .filter(|tag| tag.count > 0 && tag.tagid != -10)
                    .map(|tag| FollowingGroup {
                        tagid: tag.tagid,
                        name: tag.name,
                        count: tag.count,
                        page: 0,
                    }),
            );
        self.following.group_index = 0;
        self.following.has_more = !self.following.groups.is_empty();
        self.following.dirty = true;
        self.load_following();
    }

    pub(super) fn following_group_loaded(
        &mut self,
        request: u64,
        tagid: i64,
        page: u32,
        result: Result<model::FollowingPage, String>,
    ) {
        if self.page != Page::Following || request != self.following_request {
            return;
        }
        self.following.loading = false;
        let Some(idx) = self.following.groups.iter().position(|g| g.tagid == tagid) else {
            return;
        };
        if self.following.groups[idx].page >= page {
            return;
        }
        let data = match result {
            Ok(data) => data,
            Err(error) => {
                self.following.error = error;
                self.following.has_more = false;
                self.following.dirty = true;
                return;
            }
        };
        // 分组首页先补标题行
        if self.following.groups[idx].page == 0 {
            let header = FollowingEntry::Header {
                name: self.following.groups[idx].name.clone(),
                count: self.following.groups[idx].count,
            };
            self.following.items.push(header);
        }
        if !data.items.is_empty() {
            tasks::load_following_avatars(
                data.items.clone(),
                self.following_request,
                self.tx.clone(),
            );
        }
        self.following
            .items
            .extend(data.items.into_iter().map(FollowingEntry::User));
        self.following.groups[idx].page = page;
        if !data.has_more {
            self.following.group_index = idx + 1;
        }
        self.following.has_more =
            self.following.group_index < self.following.groups.len() || data.has_more;
        // 选中行落在标题行时向后移到首个真实用户
        if !matches!(
            self.following.items.get(self.following.selection),
            Some(FollowingEntry::User(_))
        ) {
            self.following.selection = self.following_step(self.following.selection, 1);
        }
        self.following.dirty = true;
    }

    // 移动选中行，跳过标题行，越界时停在边界
    fn following_step(&self, from: usize, delta: isize) -> usize {
        let items = &self.following.items;
        let len = items.len();
        if len == 0 || delta == 0 {
            return from;
        }
        let step = if delta > 0 { 1isize } else { -1 };
        let mut idx = from;
        for _ in 0..delta.unsigned_abs() {
            let mut next = idx as isize + step;
            while next >= 0
                && (next as usize) < len
                && matches!(items[next as usize], FollowingEntry::Header { .. })
            {
                next += step;
            }
            if next < 0 || next as usize >= len {
                break;
            }
            idx = next as usize;
        }
        idx
    }

    pub(super) fn following_key(&mut self, key: &str) {
        let previous = self.following.selection;
        match key {
            "UP" | "f" | "w" => self.following.selection = self.following_step(previous, -1),
            "DOWN" | "x" | "s" => self.following.selection = self.following_step(previous, 1),
            "LEFT" | "z" => self.following.selection = self.following_step(previous, -4),
            "RIGHT" | "c" => self.following.selection = self.following_step(previous, 4),
            "ENTER" | "SPACE" | "u" => {
                if let Some(FollowingEntry::User(user)) = self.following.items.get(previous) {
                    self.requested_uploader = Some(user.owner.clone());
                }
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
            self.following.first = ensure_visible(self.following.first, selected, 4);
            // 选中项紧邻其分组标题时，窗口上移一行让标题可见
            if selected > 0
                && matches!(
                    self.following.items[selected - 1],
                    FollowingEntry::Header { .. }
                )
            {
                self.following.first = self.following.first.min(selected - 1);
            }
            let rows = self
                .following
                .items
                .iter()
                .skip(self.following.first)
                .take(4)
                .map(|entry| match entry {
                    FollowingEntry::Header { name, count } => crate::FollowingRow {
                        user: name.as_str().into(),
                        text: format!("{count} 位关注").into(),
                        header: true,
                        ..Default::default()
                    },
                    FollowingEntry::User(user) => crate::FollowingRow {
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
                        ..Default::default()
                    },
                })
                .collect::<Vec<_>>();
            g_following.set_rows(ModelRc::new(VecModel::from(rows)));
            g_following.set_selected((selected - self.following.first) as i32);
            self.following.dirty = false;
        }
        g_following.set_loading(self.following.loading);
        g_following.set_error(self.following.error.as_str().into());
    }
}
