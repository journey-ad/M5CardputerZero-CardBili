use super::super::*;
use crate::super_chat;
use slint::Model;

impl App {
    pub(in crate::app) fn tick_super_chat(&mut self) {
        if self.live.is_none() {
            return;
        }
        let mut changed = false;
        let mut received = false;
        if let Some(connection) = &self.live_danmaku {
            for update in connection.super_chats.try_iter().take(64) {
                received |= self.super_chat.apply(update);
                changed = true;
            }
        }
        let now = super_chat::now();
        let open = self.ui.global::<crate::SuperChat>().get_open();
        changed |= self.super_chat.expire(now, open);
        let available = self
            .super_chat
            .messages
            .iter()
            .any(|item| item.visible(now));
        self.ui
            .global::<crate::SuperChat>()
            .set_available(available);
        if received && available {
            self.last_input = Instant::now();
        }
        if changed && self.ui.global::<crate::SuperChat>().get_open() {
            self.sync_super_chat();
            if self.ui.global::<crate::SuperChat>().get_rows().row_count() == 0 {
                self.close_super_chat();
                self.toast("当前暂无醒目留言");
            }
        }
        let panel = self.ui.global::<crate::SuperChat>();
        if panel.get_open() {
            panel.set_elapsed(now.saturating_sub(self.super_chat.displayed_at) as i32);
        }
        if !self.controls.danmaku_enabled {
            self.super_chat.announcements.clear();
        } else if !panel.get_open() && !self.super_chat.announcements.is_empty() {
            if let Some(player) = self
                .player
                .as_ref()
                .filter(|player| !player.paused && !player.finished && self.frames_in_seg > 0)
            {
                let time = player.position(self.frames_in_seg);
                while let Some((id, dm)) = self.super_chat.announcements.pop_front() {
                    // 播报从标题栏下方的第三条轨道开始
                    if let Err(dm) = self.live_engine.try_push(dm, time, 2) {
                        self.super_chat.announcements.push_front((id, dm));
                        break;
                    }
                }
            }
        }
    }

    pub(in crate::app) fn open_super_chat(&mut self) {
        self.tick_super_chat();
        let now = super_chat::now();
        if !self
            .super_chat
            .messages
            .iter()
            .any(|item| item.visible(now))
        {
            self.toast(if !self.super_chat.loaded {
                "醒目留言加载中…"
            } else if self.super_chat.failed {
                "醒目留言加载失败，请刷新直播间"
            } else {
                "当前暂无醒目留言"
            });
            return;
        }
        self.direction_hold = None;
        self.return_hold = None;
        self.sync_super_chat();
        let panel = self.ui.global::<crate::SuperChat>();
        panel.set_scroll(0.0);
        panel.set_open(true);
    }

    fn sync_super_chat(&mut self) {
        let now = super_chat::now();
        self.super_chat.displayed_at = now;
        let rows: Vec<_> = self
            .super_chat
            .messages
            .iter()
            .filter(|item| item.start <= now)
            .map(|item| crate::SuperChatRow {
                user: item.user.as_str().into(),
                price: format!("¥{}", item.price).into(),
                remaining: item.end.saturating_sub(now).min(i32::MAX as u64) as i32,
                text: item.text.as_str().into(),
                accent: slint::Color::from_argb_encoded(0xff000000 | item.accent),
                background: slint::Color::from_argb_encoded(0xff000000 | item.background),
                foreground: slint::Color::from_argb_encoded(0xff000000 | item.foreground),
            })
            .collect();
        let panel = self.ui.global::<crate::SuperChat>();
        panel.set_elapsed(0);
        panel.set_rows(ModelRc::new(VecModel::from(rows)));
    }

    pub(in crate::app) fn super_chat_key(&mut self, key: &str) {
        let panel = self.ui.global::<crate::SuperChat>();
        let delta = match key {
            "ESC" | "BACKSPACE" | "ENTER" | "SPACE" | "SUPER_CHAT" | "8" => {
                self.close_super_chat();
                return;
            }
            "UP" | "f" | "w" => -22.0,
            "DOWN" | "x" | "s" => 22.0,
            "LEFT" | "z" | "PGUP" => -panel.get_viewport_h(),
            "RIGHT" | "c" | "PGDN" => panel.get_viewport_h(),
            _ => return,
        };
        let max = (panel.get_content_h() - panel.get_viewport_h()).max(0.0);
        panel.set_scroll((panel.get_scroll() + delta).clamp(0.0, max));
    }

    fn close_super_chat(&mut self) {
        let now = super_chat::now();
        self.super_chat.expire(now, false);
        let panel = self.ui.global::<crate::SuperChat>();
        panel.set_open(false);
        panel.set_rows(ModelRc::default());
        panel.set_available(
            self.super_chat
                .messages
                .iter()
                .any(|item| item.visible(now)),
        );
        self.direction_hold = None;
        self.return_hold = None;
        self.last_input = Instant::now();
    }
}
