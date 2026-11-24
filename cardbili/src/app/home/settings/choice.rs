use super::*;
use crate::voice::{Backend, Punctuation};
use slint::SharedString;

impl App {
    pub(super) fn settings_choices(&self, item: SettingsItem) -> (Vec<SharedString>, i32) {
        let labels = |values: &[&'static str]| {
            values
                .iter()
                .map(|value| crate::i18n::tr(value).into())
                .collect()
        };
        match item {
            SettingsItem::MenuPosition => (
                labels(&["顶部", "左侧", "右侧"]),
                self.settings.menu_position.index(),
            ),
            SettingsItem::ListStyle => (labels(&["列表", "3列", "2列"]), self.view.index()),
            SettingsItem::DanmakuOpacity => (
                (0..=10)
                    .map(|value| format!("{}%", value * 10).into())
                    .collect(),
                self.settings.danmaku_opacity / 10,
            ),
            SettingsItem::EmojiStyle => (
                vec!["😀😆😎".into(); 2],
                i32::from(!self.settings.color_emoji),
            ),
            SettingsItem::Language => (
                labels(&["简体中文", "English"]),
                i32::from(self.settings.language == crate::i18n::Language::En),
            ),
            SettingsItem::TitleTranslate => (
                labels(&["Google", "Bing"]),
                self.settings.translate_provider,
            ),
            SettingsItem::VoiceBackend => {
                let all = [Backend::Qwen, Backend::Doubao, Backend::Wechat];
                (
                    all.iter().map(|backend| backend.label().into()).collect(),
                    all.iter()
                        .position(|backend| *backend == self.settings.voice_backend)
                        .unwrap() as i32,
                )
            }
            SettingsItem::VoicePunctuation => {
                let all = [
                    Punctuation::Full,
                    Punctuation::OmitTrailing,
                    Punctuation::Spaces,
                ];
                (
                    all.iter().map(|mode| mode.label().into()).collect(),
                    all.iter()
                        .position(|mode| *mode == self.settings.voice_punctuation)
                        .unwrap() as i32,
                )
            }
            SettingsItem::ScaleAlgorithm => (
                crate::settings::ScaleAlgorithm::ALL
                    .iter()
                    .map(|algorithm| algorithm.label().into())
                    .collect(),
                self.settings.scale_algorithm.index(),
            ),
            _ => unreachable!(),
        }
    }

    pub(super) fn apply_settings_choice(&mut self, item: SettingsItem, selected: i32) {
        match item {
            SettingsItem::MenuPosition => {
                self.settings.menu_position = MenuPosition::from_index(selected)
            }
            SettingsItem::ListStyle => self.view = VideoLayout::from_index(selected),
            SettingsItem::DanmakuOpacity => self.settings.danmaku_opacity = selected * 10,
            SettingsItem::EmojiStyle => {
                self.settings.color_emoji = selected == 0;
                self.danmaku_view.set_color_emoji(self.settings.color_emoji);
            }
            SettingsItem::Language => {
                let language = crate::i18n::Language::Zh.cycle(selected);
                self.settings.language = language;
                crate::app::set_language(&self.ui, language);
                // 底栏文案按动作缓存，切换语言后需重新生成
                self.menu_view = None;
                self.home_dirty = true;
                self.collection_dirty = true;
            }
            SettingsItem::TitleTranslate => {
                self.settings.translate_provider = selected;
                crate::title_i18n::set_provider(selected);
            }
            SettingsItem::VoiceBackend => {
                self.settings.voice_backend = Backend::Qwen.cycle(selected)
            }
            SettingsItem::VoicePunctuation => {
                self.settings.voice_punctuation = Punctuation::Full.cycle(selected)
            }
            SettingsItem::ScaleAlgorithm => {
                self.settings.scale_algorithm =
                    crate::settings::ScaleAlgorithm::Neighbor.cycle(selected)
            }
            _ => unreachable!(),
        }
        self.settings_changed = true;
    }

    pub(in crate::app) fn settings_choice_key(&mut self, key: &str) {
        let panel = self.ui.global::<crate::SettingsChoice>();
        let selected = panel.get_selected();
        let count = panel.get_options().row_count() as i32;
        match key {
            "UP" | "f" | "w" | "LEFT" | "z" | "BACKTAB" => {
                panel.set_selected((selected - 1).rem_euclid(count));
            }
            "DOWN" | "x" | "s" | "RIGHT" | "c" | "TAB" => {
                panel.set_selected((selected + 1).rem_euclid(count));
            }
            "ENTER" | "SPACE" | "7" => {
                let item = panel.get_item();
                let changed = selected != panel.get_current();
                panel.set_open(false);
                if changed {
                    self.apply_settings_choice(item, selected);
                }
                return;
            }
            "ESC" | "ESC_HOLD" | "BACKSPACE" | "8" => {
                panel.set_open(false);
                return;
            }
            _ => return,
        }
        panel.set_first(ensure_visible(
            panel.get_first() as usize,
            panel.get_selected() as usize,
            panel.get_visible_count() as usize,
        ) as i32);
    }
}
