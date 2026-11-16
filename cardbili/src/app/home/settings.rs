use super::super::*;
use crate::SettingsItem;
use slint::ComponentHandle as _;
use slint::Model;

impl App {
    pub(in crate::app) fn settings_key(&mut self, key: &str) {
        let preferences = self.ui.global::<crate::Preferences>();
        let settings_page = self.ui.global::<crate::SettingsPage>();
        let appearance = settings_page.get_appearance_items();
        let general = settings_page.get_general_items();
        let advanced = settings_page.get_advanced_items();
        let appearance_len = appearance.row_count() as i32;
        let general_len = general.row_count() as i32;
        let total = appearance_len + general_len + advanced.row_count() as i32;
        let row = if self.sel < appearance_len {
            appearance.row_data(self.sel as usize)
        } else if self.sel < appearance_len + general_len {
            general.row_data((self.sel - appearance_len) as usize)
        } else {
            advanced.row_data((self.sel - appearance_len - general_len) as usize)
        };
        let Some(row) = row else {
            return;
        };
        // 上下方向键在首尾设置项之间循环
        let step = match key {
            "UP" => {
                self.sel = (self.sel - 1).rem_euclid(total);
                return;
            }
            "DOWN" => {
                self.sel = (self.sel + 1).rem_euclid(total);
                return;
            }
            "ESC" | "BACKSPACE" => {
                self.tab = HomeTab::Recommended;
                self.sel = 0;
                return;
            }
            "LEFT" => -1,
            "RIGHT" => 1,
            "ENTER" => 0,
            _ => return,
        };
        match row.item {
            SettingsItem::ListStyle if step != 0 => {
                self.view = self.view.cycle(step);
                self.settings_changed = true;
            }
            SettingsItem::DanmakuOpacity if step != 0 => {
                let opacity = (self.settings.danmaku_opacity + step * 10).clamp(0, 100);
                self.settings_changed |= opacity != self.settings.danmaku_opacity;
                self.settings.danmaku_opacity = opacity;
            }
            SettingsItem::MenuPosition if step != 0 => {
                self.settings.menu_position = self.settings.menu_position.cycle(step);
                self.settings_changed = true;
            }
            SettingsItem::ScaleAlgorithm if step != 0 && !self.settings.pixel_optimize => {
                self.settings.scale_algorithm = self.settings.scale_algorithm.cycle(step);
                self.settings_changed = true;
            }
            SettingsItem::Danmaku => self.toggle_danmaku(),
            SettingsItem::RecordHistory => self.toggle_record_history(),
            SettingsItem::DebugLog => self.toggle_debug_log(),
            SettingsItem::EmojiStyle => self.toggle_color_emoji(),
            SettingsItem::PixelOptimize => {
                self.settings.pixel_optimize = !self.settings.pixel_optimize;
                self.settings_changed = true;
            }
            SettingsItem::Cdn if step == 0 => self.open_cdn(),
            SettingsItem::Cache if step == 0 => {
                if !preferences.get_cache_busy() {
                    self.confirm(
                        confirmation::Action::ClearCache,
                        "清理缓存",
                        "是否确认清理图片和接口缓存？\n登录状态和设置将会保留",
                        "清理",
                    );
                }
            }
            SettingsItem::About if step == 0 => self.requested_about = true,
            _ => {}
        }
    }

    // 选中行超出视口时滚动，分组首项同时显示标题
    pub(in crate::app) fn sync_settings_scroll(&self) {
        let settings_page = self.ui.global::<crate::SettingsPage>();
        let appearance_len = settings_page.get_appearance_items().row_count() as i32;
        let general_len = settings_page.get_general_items().row_count() as i32;
        let sel = self.sel;
        // 该行在设置列表里的绝对行号，外观/常规/进阶各占一个标题行
        let line = sel
            + 1
            + if sel >= appearance_len { 1 } else { 0 }
            + if sel >= appearance_len + general_len {
                1
            } else {
                0
            };
        let video_h = self.ui.global::<crate::Layout>().get_video_height();
        let visible = visible_rows(video_h, 4.0, 24.0);
        // 选中某组第一项时，把该组标题一并留在视口内
        let mut first = ensure_visible(
            settings_page.get_first_line() as usize,
            line as usize,
            visible,
        );
        if sel == 0 || sel == appearance_len || sel == appearance_len + general_len {
            first = first.min((line - 1) as usize);
        }
        settings_page.set_first_line(first as i32);
    }

    fn toggle_color_emoji(&mut self) {
        self.settings.color_emoji = !self.settings.color_emoji;
        self.danmaku_view.set_color_emoji(self.settings.color_emoji);
        self.settings_changed = true;
    }

    fn toggle_record_history(&mut self) {
        self.settings.record_history = !self.settings.record_history;
        self.history.set_enabled(self.settings.record_history);
        self.settings_changed = true;
    }

    fn toggle_debug_log(&mut self) {
        let enabled = !self.settings.debug_log;
        match crate::diagnostics::set_enabled(enabled) {
            Ok(()) => {
                self.settings.debug_log = enabled;
                self.settings_changed = true;
                if enabled {
                    self.toast(&format!(
                        "日志目录 {}",
                        crate::diagnostics::path().parent().unwrap().display()
                    ));
                }
            }
            Err(error) => self.toast(&format!("调试日志设置失败：{error}")),
        }
    }
}
