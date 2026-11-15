use crate::{diagnostics, MenuPosition, VideoLayout};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    pub list_view: VideoLayout,
    pub danmaku_enabled: bool,
    pub danmaku_opacity: i32,
    pub menu_position: MenuPosition,
    pub record_history: bool,
    pub debug_log: bool,
    pub color_emoji: bool,
    pub cdn: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            list_view: VideoLayout::ThreeColumns,
            danmaku_enabled: true,
            danmaku_opacity: 90,
            menu_position: MenuPosition::Top,
            record_history: true,
            debug_log: false,
            color_emoji: true,
            cdn: "backup".into(),
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let mut settings: Self = std::fs::read(Self::path())
            .map_err(|error| error.to_string())
            .and_then(|data| serde_json::from_slice(&data).map_err(|error| error.to_string()))
            .unwrap_or_else(|error| {
                diagnostics::write(format_args!("[设置] 使用默认值，读取失败: {error}"));
                Self::default()
            });
        settings.danmaku_opacity = settings.danmaku_opacity.clamp(0, 100);
        settings.cdn = crate::cdn::OPTIONS[crate::cdn::index(&settings.cdn)]
            .id
            .into();
        settings
    }

    pub fn save(&self) -> Result<(), String> {
        let result = (|| {
            let path = Self::path();
            if let Some(directory) = path.parent() {
                std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
            }
            let data = serde_json::to_vec_pretty(self).map_err(|error| error.to_string())?;
            std::fs::write(path, data).map_err(|error| error.to_string())
        })();
        diagnostics::write(format_args!("[设置] 保存 {self:?} 结果={result:?}"));
        result
    }

    fn path() -> PathBuf {
        crate::util::config_dir().join("settings.json")
    }
}

impl VideoLayout {
    pub fn from_index(index: i32) -> Self {
        match index.clamp(0, 2) {
            0 => Self::List,
            1 => Self::ThreeColumns,
            _ => Self::TwoColumns,
        }
    }

    pub fn index(self) -> i32 {
        match self {
            Self::List => 0,
            Self::ThreeColumns => 1,
            Self::TwoColumns => 2,
        }
    }

    pub fn cycle(self, step: i32) -> Self {
        Self::from_index((self.index() + step).rem_euclid(3))
    }
}

impl Serialize for VideoLayout {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i32(self.index())
    }
}

impl<'de> Deserialize<'de> for VideoLayout {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        i32::deserialize(deserializer).map(Self::from_index)
    }
}

impl MenuPosition {
    pub fn from_index(index: i32) -> Self {
        match index.clamp(0, 2) {
            0 => Self::Top,
            1 => Self::Left,
            _ => Self::Right,
        }
    }

    pub fn index(self) -> i32 {
        match self {
            Self::Top => 0,
            Self::Left => 1,
            Self::Right => 2,
        }
    }

    pub fn cycle(self, step: i32) -> Self {
        Self::from_index((self.index() + step).rem_euclid(3))
    }
}

impl Serialize for MenuPosition {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i32(self.index())
    }
}

impl<'de> Deserialize<'de> for MenuPosition {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        i32::deserialize(deserializer).map(Self::from_index)
    }
}
