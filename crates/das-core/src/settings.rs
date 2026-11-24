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

    // BitTorrent
    /// Port for incoming peers. Picked at random once, then kept so seeding keeps working.
    pub torrent_port: u16,
    pub torrent_upnp: bool,
    pub torrent_dht: bool,
    /// Use uTP (BitTorrent over UDP) as well as TCP.
    pub torrent_utp: bool,
    /// While anything downloads, hold torrent uploads to 80% of the line's upload speed.
    pub protect_downloads: bool,
    /// Upload limit for torrents, bytes per second, 0 = unlimited.
    pub upload_limit: u64,
    /// Stop seeding once uploaded / size reaches this (0 = no ratio limit).
    pub seed_ratio: f32,
    /// Stop seeding after this many minutes (0 = no time limit).
    pub seed_minutes: u32,
    /// Trackers added to every new torrent, one per line.
    pub default_trackers: String,
}

impl Settings {
    pub fn default_tracker_list(&self) -> Vec<String> {
        self.default_trackers.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect()
    }
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
            torrent_port: 0,
            torrent_upnp: true,
            torrent_dht: true,
            torrent_utp: true,
            protect_downloads: true,
            upload_limit: 0,
            seed_ratio: 1.0,
            seed_minutes: 0,
            default_trackers: String::new(),
        }
    }
}
