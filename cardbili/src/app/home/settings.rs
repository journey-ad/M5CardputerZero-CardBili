use super::super::*;
use crate::SettingsItem;
use slint::Model;

impl App {
    pub(in crate::app) fn settings_key(&mut self, key: &str) {
        let appearance = self.ui.get_appearance_items();
        let general = self.ui.get_general_items();
        let appearance_len = appearance.row_count() as i32;
        let total = appearance_len + general.row_count() as i32;
        let row = if self.sel < appearance_len {
            appearance.row_data(self.sel as usize)
        } else {
            general.row_data((self.sel - appearance_len).max(0) as usize)
        };
        let Some(row) = row else {
            return;
        };
        match key {
            "UP" => self.sel = (self.sel - 1).max(0),
            "DOWN" => self.sel = (self.sel + 1).min(total - 1),
            "LEFT" | "RIGHT" => {
                let step = if key == "LEFT" { -1 } else { 1 };
                match row.item {
                    SettingsItem::ListStyle => {
                        self.view = self.view.cycle(step);
                        self.settings_changed = true;
                    }
                    SettingsItem::Danmaku => self.toggle_danmaku(),
                    SettingsItem::DanmakuOpacity => {
                        let opacity = (self.settings.danmaku_opacity + step * 10).clamp(0, 100);
                        self.settings_changed |= opacity != self.settings.danmaku_opacity;
                        self.settings.danmaku_opacity = opacity;
                    }
                    SettingsItem::MenuPosition => {
                        self.settings.menu_position = self.settings.menu_position.cycle(step);
                        self.settings_changed = true;
                    }
                    SettingsItem::RecordHistory => self.toggle_record_history(),
                    SettingsItem::DebugLog => self.toggle_debug_log(),
                    _ => {}
                }
            }
            "ENTER" => match row.item {
                SettingsItem::Danmaku => self.toggle_danmaku(),
                SettingsItem::RecordHistory => self.toggle_record_history(),
                SettingsItem::DebugLog => self.toggle_debug_log(),
                SettingsItem::Cdn => self.open_cdn(),
                SettingsItem::Cache => {
                    if !self.ui.get_cache_busy() {
                        self.confirm(
                            confirmation::Action::ClearCache,
                            "清理缓存",
                            "清理图片和接口缓存？\n登录状态和设置会保留",
                            "清理",
                        );
                    }
                }
                SettingsItem::About => self.requested_about = true,
                _ => {}
            },
            "ESC" | "BACKSPACE" => {
                self.tab = HomeTab::Recommended;
                self.sel = 0;
            }
            _ => {}
        }
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
