use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub download_dir: String,
    /// Put each category in its own subfolder (Downloads\Video…).
    pub category_folders: bool,
    pub max_concurrent: usize,
    pub connections: u32,
    /// Bytes per second, 0 = unlimited.
    pub speed_limit: u64,
    pub close_to_tray: bool,
    pub notify_on_complete: bool,
    pub start_with_windows: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let dir = dirs::download_dir().unwrap_or_else(|| std::env::temp_dir().join("Downloads"));
        Self {
            download_dir: dir.to_string_lossy().into_owned(),
            category_folders: true,
            max_concurrent: 4,
            connections: 16,
            speed_limit: 0,
            close_to_tray: true,
            notify_on_complete: true,
            start_with_windows: false,
        }
    }
}
