//! DasIDM application core: persistence, queue, categories, settings. UI-agnostic.

pub mod categories;
mod db;
pub mod links;
mod manager;
pub mod model;
pub mod settings;
pub mod tools;
pub mod video;

pub use das_engine::ProbeInfo;
pub use manager::{Emit, Manager};
pub use model::{AddRequest, CoreEvent, ItemView, Progress, Status, Totals};
pub use settings::Settings;
