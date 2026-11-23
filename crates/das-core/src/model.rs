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
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
