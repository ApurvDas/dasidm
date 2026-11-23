use crate::video::{VideoOption, VideoPhase, VideoSpec};
use das_engine::ResumeState;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Queued,
    Downloading,
    Paused,
    Done,
    Failed,
}

/// What kind of job a download is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A file, fetched by DasIDM's own multi-connection engine.
    #[default]
    Http,
    /// A video or music link, fetched by yt-dlp.
    Video,
    /// A torrent or magnet link, fetched by the BitTorrent session.
    Torrent,
}

/// What a torrent item remembers.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentSpec {
    pub info_hash: String,
    pub name: String,
    pub files: usize,
    #[serde(default)]
    pub private: bool,
    /// Indexes of the chosen files; None = all.
    #[serde(default)]
    pub only_files: Option<Vec<usize>>,
    /// Where the data lives (the torrent's own folder for multi-file torrents).
    pub output_dir: String,
    #[serde(default)]
    pub uploaded: u64,
    /// Finished and still uploading to others.
    #[serde(default)]
    pub seeding: bool,
    /// When seeding started (for the time limit).
    #[serde(default)]
    pub seed_since: Option<i64>,
    #[serde(default)]
    pub finished_once: bool,
}

/// A download as stored in the database.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub url: String,
    pub file_name: String,
    pub save_dir: String,
    pub file_path: Option<String>,
    pub size: Option<u64>,
    pub downloaded: u64,
    pub status: Status,
    pub category: String,
    pub added_at: i64,
    pub finished_at: Option<i64>,
    pub error: Option<String>,
    pub connections: u32,
    pub resumable: bool,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    #[serde(default)]
    pub resume: Option<ResumeState>,
    /// Order inside the queue; lower starts first.
    #[serde(default)]
    pub position: i64,
    /// The user chose the name; do not replace it with the server's.
    #[serde(default)]
    pub name_fixed: bool,
    /// The user confirmed they want the web page itself saved.
    #[serde(default)]
    pub allow_html: bool,
    #[serde(default)]
    pub kind: Kind,
    #[serde(default)]
    pub video: Option<VideoSpec>,
    #[serde(default)]
    pub torrent: Option<TorrentSpec>,
}

/// What the UI sees for one row.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemView {
    pub id: String,
    pub url: String,
    pub file_name: String,
    pub save_dir: String,
    pub file_path: Option<String>,
    pub size: Option<u64>,
    pub downloaded: u64,
    pub status: Status,
    pub category: String,
    pub added_at: i64,
    pub finished_at: Option<i64>,
    pub error: Option<String>,
    pub connections: u32,
    pub resumable: bool,
    pub kind: Kind,
    /// Video and music links: what was chosen and what it is.
    pub media: Option<MediaView>,
    pub torrent: Option<TorrentView>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentView {
    pub info_hash: String,
    pub files: usize,
    pub chosen_files: Option<usize>,
    pub private: bool,
    pub uploaded: u64,
    /// Uploaded / size.
    pub ratio: f32,
    pub seeding: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaView {
    pub title: String,
    /// "1080p", "MP3".
    pub quality: String,
    /// "MP4", "MP3".
    pub format: String,
    pub audio_only: bool,
    pub duration: Option<u64>,
    pub channel: Option<String>,
    pub site: Option<String>,
    /// A thumbnail is stored; ask for it with `thumbnail(id)`.
    pub thumb: bool,
}

impl From<&Item> for ItemView {
    fn from(i: &Item) -> Self {
        Self {
            id: i.id.clone(),
            url: i.url.clone(),
            file_name: i.file_name.clone(),
            save_dir: i.save_dir.clone(),
            file_path: i.file_path.clone(),
            size: i.size,
            downloaded: i.downloaded,
            status: i.status,
            category: i.category.clone(),
            added_at: i.added_at,
            finished_at: i.finished_at,
            error: i.error.clone(),
            connections: i.connections,
            resumable: i.resumable,
            kind: i.kind,
            media: i.video.as_ref().map(|v| MediaView {
                title: v.title.clone(),
                quality: v.quality.clone(),
                format: v.ext.to_uppercase(),
                audio_only: matches!(v.choice, crate::video::Choice::Audio { .. }),
                duration: v.duration,
                channel: v.channel.clone(),
                site: v.site.clone(),
                thumb: v.thumb.is_some(),
            }),
            torrent: i.torrent.as_ref().map(|t| TorrentView {
                info_hash: t.info_hash.clone(),
                files: t.files,
                chosen_files: t.only_files.as_ref().map(Vec::len),
                private: t.private,
                uploaded: t.uploaded,
                ratio: match i.size {
                    Some(s) if s > 0 => t.uploaded as f32 / s as f32,
                    _ => 0.0,
                },
                seeding: t.seeding,
            }),
        }
    }
}

/// Live numbers for one running download.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub id: String,
    pub downloaded: u64,
    pub size: Option<u64>,
    /// Bytes per second, smoothed.
    pub speed: u64,
    /// Seconds left, if known.
    pub eta: Option<u64>,
    pub active_connections: usize,
    /// Byte ranges still missing, as `[from, to]` fractions of the file.
    pub gaps: Vec<[f32; 2]>,
    /// Where each live connection is writing right now (fractions).
    pub heads: Vec<f32>,
    /// The parts being downloaded right now, one per live connection, in file order.
    pub parts: Vec<PartView>,
    /// How many parts the file has been split into so far (finished ones included).
    pub parts_total: usize,
    pub file_name: String,
    /// Video jobs: what yt-dlp is doing (downloading, merging, converting…).
    pub phase: Option<VideoPhase>,
    /// Torrents: the swarm.
    pub torrent: Option<TorrentLive>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentLive {
    pub up_speed: u64,
    pub uploaded: u64,
    pub peers: u32,
    pub seeds: u32,
    pub known_peers: u32,
    /// Checking pieces already on disk.
    pub checking: bool,
    pub seeding: bool,
    /// Verified share of each stretch of the torrent (0..1), for the piece map.
    pub pieces: Vec<f32>,
}

/// One byte range served by one connection.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PartView {
    /// Where the part starts, how far it is written, and where it ends (bytes).
    pub start: u64,
    pub pos: u64,
    pub end: u64,
    /// Bytes per second on this connection, smoothed.
    pub speed: u64,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub speed: u64,
    pub active: usize,
    pub queued: usize,
    pub remaining_bytes: u64,
    pub eta: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddRequest {
    pub url: String,
    #[serde(default)]
    pub file_name: Option<String>,
    #[serde(default)]
    pub save_dir: Option<String>,
    #[serde(default)]
    pub connections: Option<u32>,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    /// false = add paused.
    #[serde(default = "yes")]
    pub start: bool,
    /// Save the link even if it turns out to be a web page.
    #[serde(default)]
    pub allow_page: bool,
}

/// Add a torrent that was previewed (by its info hash) with the chosen files.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddTorrentRequest {
    pub info_hash: String,
    #[serde(default)]
    pub only_files: Option<Vec<usize>>,
    /// Extra trackers, one per entry (the default list from Settings is added too).
    #[serde(default)]
    pub extra_trackers: Vec<String>,
    #[serde(default)]
    pub save_dir: Option<String>,
    #[serde(default = "yes")]
    pub start: bool,
}

/// Add a video or music link with a choice from the quality picker.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddVideoRequest {
    pub url: String,
    pub option: VideoOption,
    pub title: String,
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub duration: Option<u64>,
    #[serde(default)]
    pub thumbnail: Option<String>,
    #[serde(default)]
    pub site: Option<String>,
    #[serde(default)]
    pub save_dir: Option<String>,
    #[serde(default = "yes")]
    pub start: bool,
    /// Add every video of the link's playlist with the same choice.
    #[serde(default)]
    pub playlist: bool,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CoreEvent {
    Upsert { item: ItemView },
    Removed { ids: Vec<String> },
    Progress { items: Vec<Progress>, totals: Totals },
    Completed { item: ItemView },
    /// The video helpers changed (installed, installing, failed).
    Tools { status: crate::tools::ToolsStatus },
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
