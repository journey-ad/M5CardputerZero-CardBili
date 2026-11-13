use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    pub list_view: i32,
    pub danmaku_enabled: bool,
    pub danmaku_opacity: i32,
    pub menu_position: i32,
    pub record_history: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            list_view: 1,
            danmaku_enabled: true,
            danmaku_opacity: 90,
            menu_position: 0,
            record_history: true,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let mut settings: Self = std::fs::read(Self::path())
            .ok()
            .and_then(|data| {
                let mut value: serde_json::Value = serde_json::from_slice(&data).ok()?;
                if value.get("danmaku_opacity").is_none() {
                    if let Some(transparency) = value["danmaku_transparency"].as_i64() {
                        value["danmaku_opacity"] = (100 - transparency.clamp(0, 100)).into();
                    }
                }
                serde_json::from_value(value).ok()
            })
            .unwrap_or_default();
        settings.list_view = settings.list_view.clamp(0, 2);
        settings.danmaku_opacity = settings.danmaku_opacity.clamp(0, 100);
        settings.menu_position = settings.menu_position.clamp(0, 2);
        settings
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::path();
        if let Some(directory) = path.parent() {
            std::fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        }
        let data = serde_json::to_vec_pretty(self).map_err(|error| error.to_string())?;
        std::fs::write(path, data).map_err(|error| error.to_string())
    }

    fn path() -> PathBuf {
        PathBuf::from(crate::util::cookie_path()).with_file_name("settings.json")
    }
}
