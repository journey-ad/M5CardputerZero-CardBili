use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    pub list_view: i32,
    pub danmaku_transparency: i32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            list_view: 1,
            danmaku_transparency: 10,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let mut settings: Self = std::fs::read(Self::path())
            .ok()
            .and_then(|data| serde_json::from_slice(&data).ok())
            .unwrap_or_default();
        settings.list_view = settings.list_view.clamp(0, 2);
        settings.danmaku_transparency = settings.danmaku_transparency.clamp(0, 100);
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
