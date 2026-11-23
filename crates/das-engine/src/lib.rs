//! DasIDM download engine: segmented, multi-connection HTTP(S) with dynamic work stealing.
//!
//! One file is split into ranges served by up to N connections. The first request
//! (`Range: bytes=0-`) doubles as the probe and becomes the first segment; every idle
//! connection then steals half of the largest remaining range, so all connections stay
//! busy until the last byte. Data goes straight to its final offset in `<name>.dasdl`,
//! which is renamed when complete.

mod download;
pub mod filename;
mod limiter;
mod storage;

pub use download::{DownloadHandle, Phase, SegmentView, Snapshot};
pub use limiter::RateLimiter;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DownloadRequest {
    pub url: String,
    /// Extra headers: Cookie, Referer, User-Agent, Authorization…
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    pub save_dir: PathBuf,
    /// Overrides the detected name.
    #[serde(default)]
    pub file_name: Option<String>,
    pub connections: usize,
}

/// Everything needed to continue a stopped download.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct ResumeState {
    pub final_url: String,
    pub size: Option<u64>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub file_path: PathBuf,
    pub resumable: bool,
    /// Byte ranges still missing, `[start, end)`.
    pub remaining: Vec<(u64, u64)>,
}

impl ResumeState {
    pub fn downloaded(&self) -> u64 {
        let left: u64 = self.remaining.iter().map(|(a, b)| b - a).sum();
        self.size.map(|s| s.saturating_sub(left)).unwrap_or(0)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProbeInfo {
    pub final_url: String,
    pub file_name: String,
    pub size: Option<u64>,
    pub resumable: bool,
    pub content_type: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Completed {
    pub path: PathBuf,
    pub size: u64,
}

#[derive(Clone, Debug, thiserror::Error, Serialize, Deserialize, PartialEq)]
pub enum EngineError {
    #[error("server answered HTTP {0}")]
    Http(u16),
    #[error("network error: {0}")]
    Network(String),
    #[error("disk error: {0}")]
    Io(String),
    #[error("the file on the server changed since the download started")]
    ValidatorMismatch,
    #[error("the server stopped sending data")]
    NoProgress,
    #[error("paused")]
    Paused,
    #[error("cancelled")]
    Cancelled,
    #[error("invalid address: {0}")]
    BadUrl(String),
}

impl EngineError {
    /// Errors worth retrying with backoff.
    pub fn is_transient(&self) -> bool {
        match self {
            EngineError::Network(_) | EngineError::NoProgress => true,
            EngineError::Http(code) => matches!(code, 408 | 425 | 429 | 500..=599),
            _ => false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct EngineConfig {
    pub user_agent: String,
    pub connect_timeout: Duration,
    /// Max silence on a connection before it is recycled.
    pub read_timeout: Duration,
    /// Consecutive failures without progress before a connection gives up.
    pub max_retries: u32,
    /// A range is only split if each half would be at least this big.
    pub min_split: u64,
    /// Bytes buffered per connection before one positioned write.
    pub buffer_size: usize,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            user_agent: format!("DasIDM/{}", env!("CARGO_PKG_VERSION")),
            connect_timeout: Duration::from_secs(15),
            read_timeout: Duration::from_secs(30),
            max_retries: 8,
            min_split: 1024 * 1024,
            buffer_size: 512 * 1024,
        }
    }
}

/// Cheap to clone.
#[derive(Clone)]
pub struct Engine {
    pub(crate) config: Arc<EngineConfig>,
    global: Arc<RateLimiter>,
}

impl Engine {
    pub fn new(config: EngineConfig) -> Self {
        Self { config: Arc::new(config), global: Arc::new(RateLimiter::unlimited()) }
    }

    pub fn global_limiter(&self) -> &Arc<RateLimiter> {
        &self.global
    }

    /// Start (or resume) a download. Must be called inside a tokio runtime.
    pub fn start(
        &self,
        req: DownloadRequest,
        resume: Option<ResumeState>,
        extra_limiters: Vec<Arc<RateLimiter>>,
    ) -> DownloadHandle {
        let mut limiters = vec![self.global.clone()];
        limiters.extend(extra_limiters);
        download::spawn(self.config.clone(), req, resume, limiters)
    }

    /// Ask the server for name, size and range support without downloading.
    pub async fn probe(&self, req: &DownloadRequest) -> Result<ProbeInfo, EngineError> {
        download::probe(&self.config, req).await
    }
}
