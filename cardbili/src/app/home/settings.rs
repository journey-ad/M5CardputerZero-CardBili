use super::super::*;
use crate::SettingsItem;
use slint::ComponentHandle as _;
use slint::Model;

mod choice;

impl App {
    pub(in crate::app) fn settings_key(&mut self, key: &str) {
        let preferences = self.ui.global::<crate::Preferences>();
        let settings_page = self.ui.global::<crate::SettingsPage>();
        let groups: Vec<_> = settings_page.get_groups().iter().collect();
        let total = groups.iter().map(|group| group.rows.row_count()).sum::<usize>() as i32;
        let row = groups
            .iter()
            .flat_map(|group| group.rows.iter())
            .nth(self.sel as usize);
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
            "ENTER" | "SPACE" => 0,
            _ => return,
        };
        if row.kind == crate::SettingsKind::Choice {
            if row.item == SettingsItem::ScaleAlgorithm && self.settings.pixel_optimize {
                return;
            }
            let (options, current) = self.settings_choices(row.item);
            if step == 0 {
                let panel = self.ui.global::<crate::SettingsChoice>();
                panel.set_item(row.item);
                panel.set_title(row.label);
                panel.set_options(ModelRc::new(VecModel::from(options)));
                panel.set_current(current);
                panel.set_selected(current);
                panel.set_first((current - panel.get_visible_count() + 1).max(0));
                panel.set_open(true);
                self.direction_hold = None;
                self.return_hold = None;
            } else {
                let count = options.len() as i32;
                let selected = if row.item == SettingsItem::DanmakuOpacity {
                    (current + step).clamp(0, count - 1)
                } else {
                    (current + step).rem_euclid(count)
                };
                if selected != current {
                    self.apply_settings_choice(row.item, selected);
                }
            }
            return;
        }
        match row.item {
            SettingsItem::VoiceAddSpaces => {
                self.settings.voice_add_spaces = !self.settings.voice_add_spaces;
                self.settings_changed = true;
            }
            SettingsItem::Danmaku => {
                self.settings.danmaku_enabled = !self.settings.danmaku_enabled;
                self.settings_changed = true;
            }
            SettingsItem::RecordHistory => self.toggle_record_history(),
            SettingsItem::DebugLog => self.toggle_debug_log(),
            SettingsItem::PixelOptimize => {
                self.settings.pixel_optimize = !self.settings.pixel_optimize;
                self.settings_changed = true;
            }
            SettingsItem::Cdn if step == 0 => self.open_cdn(),
            SettingsItem::Cache if step == 0 => {
                if !preferences.get_cache_busy() {
                    self.confirm(
                        confirmation::Action::ClearCache,
                        crate::i18n::tr("清理缓存"),
                        crate::i18n::tr("是否确认清理图片和接口缓存？\n登录状态和设置将会保留"),
                        crate::i18n::tr("清理"),
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
        let video_h = self.ui.global::<crate::Layout>().get_video_height();
        let visible = visible_rows(video_h, 4.0, 24.0);
        let mut start = 0;
        for (index, group) in settings_page.get_groups().iter().enumerate() {
            let end = start + group.rows.row_count() as i32;
            if self.sel < end {
                settings_page.set_selected_group(index as i32);
                settings_page.set_selected_row(self.sel - start);
                // 各分组标题占一行
                let line = self.sel as usize + index + 1;
                let mut first = ensure_visible(settings_page.get_first_line() as usize, line, visible);
                // 选中某组第一项时，把该组标题一并留在视口内
                if self.sel == start {
                    first = first.min(line - 1);
                }
                settings_page.set_first_line(first as i32);
                break;
            }
            start = end;
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
                    self.toast(&crate::i18n::trf(
                        "日志目录 {}",
                        &[&crate::diagnostics::path().parent().unwrap().display()],
                    ));
                }
            }
            Err(error) => self.toast(&crate::i18n::trf("调试日志设置失败：{}", &[&error])),
        }
    }
}
