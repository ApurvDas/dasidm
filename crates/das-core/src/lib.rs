//! DasIDM application core: persistence, queue, categories, settings. UI-agnostic.

pub mod categories;
mod db;
pub mod links;
pub mod music;
mod manager;
pub mod model;
pub mod settings;
pub mod tools;
pub mod video;

pub use das_engine::ProbeInfo;
pub use das_torrent::{FileEntry, Preview as TorrentPreview, TrackerView};
pub use manager::{Emit, Manager, UploadLearned, UploadStatus, is_torrent_link};
pub use model::{AddRequest, CoreEvent, ItemView, Progress, Status, Totals};
pub use settings::Settings;
