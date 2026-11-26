//! Owns all downloads: queue admission, engine jobs, progress batching, checkpoints.

use crate::categories::{categorize, categorize_with, folder_for};
use crate::db::Db;
use crate::model::*;
use crate::settings::Settings;
use crate::tools::{Tools, ToolsStatus};
use crate::video::{self, VideoError, VideoInfo, VideoJob, VideoSpec};
use das_engine::filename::{file_name_from_url, part_path, sanitize_file_name, unique_path};
use das_engine::{DownloadHandle, DownloadRequest, Engine, EngineConfig, EngineError, ProbeInfo};
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[path = "torrent_jobs.rs"]
mod torrent_jobs;
pub use torrent_jobs::is_torrent_link;
#[path = "bandwidth.rs"]
mod bandwidth;
#[path = "convert_jobs.rs"]
mod convert_jobs;
pub use convert_jobs::{AddConvertRequest, ConvertPlan, PlannedFile};
pub use bandwidth::{UploadLearned, UploadStatus};

pub type Emit = Arc<dyn Fn(CoreEvent) + Send + Sync>;

#[derive(Clone)]
pub struct Manager {
    inner: Arc<Inner>,
}

struct Inner {
    engine: Engine,
    db: Mutex<Db>,
    state: Mutex<State>,
    settings: RwLock<Settings>,
    emit: Emit,
    visible: AtomicBool,
    was_running: AtomicBool,
    /// Last 60 s of total speed (4 samples per second), kept here because the window
    /// (and its memory) is destroyed whenever it closes.
    history: Mutex<std::collections::VecDeque<u64>>,
    tools: Arc<Tools>,
    data_dir: PathBuf,
    /// The BitTorrent session (started only when a torrent needs it).
    torrents: Arc<das_torrent::Torrents>,
    /// Torrents read but not added yet, by info hash.
    previews: Mutex<HashMap<String, Arc<das_torrent::Preview>>>,
    /// Since when the torrent session has had nothing to download or seed (ms, 0 = not idle).
    torrents_idle_since: std::sync::atomic::AtomicI64,
    /// When the torrent view last asked for trackers or files (ms); counts as activity.
    torrents_viewed_at: std::sync::atomic::AtomicI64,
    /// The shared speed limit and the torrent upload cap.
    bandwidth: Mutex<bandwidth::Bandwidth>,
    /// Quitting: stopped conversions stay "running" so they carry on at the next start.
    quitting: AtomicBool,
}

const HISTORY: usize = 240;

#[derive(Default)]
struct State {
    items: HashMap<String, Item>,
    running: HashMap<String, Running>,
    /// Conversion batches, kept apart from `running` so they never count as downloads.
    converting: HashMap<String, crate::convert::ConvertJob>,
    next_position: i64,
}

/// A running download: a file through the engine, or a video link through yt-dlp.
enum Job {
    Http(DownloadHandle),
    Video(VideoJob),
}

impl Job {
    fn pause(&self) {
        match self {
            Job::Http(h) => h.pause(),
            Job::Video(v) => v.pause(),
        }
    }
}

struct Running {
    job: Job,
    last_bytes: u64,
    last_at: Instant,
    speed: f64,
    /// Per-part (keyed by the part's start byte): last written position and smoothed speed.
    parts: HashMap<u64, (u64, f64)>,
}

impl Manager {
    pub fn open(data_dir: &Path, emit: Emit) -> Result<Self, String> {
        let db = Db::open(&data_dir.join("dasidm.db")).map_err(|e| e.to_string())?;
        let mut settings = db.load_settings();
        // A fixed port keeps seeding reachable (UPnP maps it once); pick one the first time.
        if settings.torrent_port == 0 {
            settings.torrent_port = fastrand::u16(20000..60000);
            let _ = db.save_settings(&settings);
        }
        let mut state = State::default();
        for mut item in db.load_items().map_err(|e| e.to_string())? {
            // A crash or exit mid-download: pick it up again. Torrents stay Downloading: the
            // torrent session resumes them (`start_torrents_if_needed`); the HTTP queue can't.
            if item.status == Status::Downloading && item.kind != Kind::Torrent {
                item.status = Status::Queued;
            }
            // A finished video left in its work folder (older versions could lose the real file
            // name): move it next to the user's files now.
            if item.kind == Kind::Video && item.status == Status::Done {
                let stuck = item.file_path.as_deref().is_none_or(|p| !Path::new(p).is_file() || Path::new(p).components().any(|c| c.as_os_str() == WORK_DIR));
                if stuck && video_work_dir(&item.save_dir, &item.id).is_dir() {
                    let made = item.file_path.clone().map(PathBuf::from).unwrap_or_default();
                    let path = place_video(&item.save_dir, &item.id, &made);
                    if path.is_file() {
                        item.file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        item.file_path = Some(path.to_string_lossy().into_owned());
                        let _ = db.save_item(&item);
                    }
                }
            }
            state.next_position = state.next_position.max(item.position + 1);
            state.items.insert(item.id.clone(), item);
        }
        let engine = Engine::new(EngineConfig::default());
        engine.global_limiter().set_rate(settings.speed_limit);
        let tools = Tools::new(emit.clone());
        let torrents = Arc::new(das_torrent::Torrents::new(&data_dir.join("torrents"), torrent_jobs::torrent_settings(&settings)));
        Ok(Self {
            inner: Arc::new(Inner {
                engine,
                db: Mutex::new(db),
                state: Mutex::new(state),
                settings: RwLock::new(settings),
                emit,
                visible: AtomicBool::new(true),
                was_running: AtomicBool::new(false),
                history: Mutex::new(std::collections::VecDeque::with_capacity(HISTORY)),
                tools,
                data_dir: data_dir.to_path_buf(),
                torrents,
                previews: Mutex::new(HashMap::new()),
                torrents_idle_since: std::sync::atomic::AtomicI64::new(0),
                torrents_viewed_at: std::sync::atomic::AtomicI64::new(0),
                bandwidth: Mutex::new(bandwidth::Bandwidth::load(data_dir)),
                quitting: AtomicBool::new(false),
            }),
        })
    }

    /// Background loop: progress at 4 Hz (only while visible), checkpoints every 2 s.
    /// Must run on a tokio runtime; it also starts queued downloads.
    pub async fn run(self) {
        self.inner.schedule();
        self.inner.start_torrents_if_needed();
        let mut tick = tokio::time::interval(Duration::from_millis(250));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut n: u64 = 0;
        loop {
            tick.tick().await;
            n += 1;
            let any_running = self.inner.progress_tick();
            if any_running && n % 8 == 0 {
                self.inner.checkpoint().await;
            }
            if n % 8 == 0 {
                self.inner.stop_idle_torrents().await;
                self.inner.balance_bandwidth();
            }
        }
    }

    pub fn set_visible(&self, visible: bool) {
        self.inner.visible.store(visible, Ordering::Relaxed);
    }

    pub fn list(&self) -> Vec<ItemView> {
        let st = self.inner.state.lock();
        let mut v: Vec<ItemView> = st.items.values().map(ItemView::from).collect();
        v.sort_by(|a, b| b.added_at.cmp(&a.added_at));
        v
    }

    pub fn settings(&self) -> Settings {
        self.inner.settings.read().clone()
    }

    /// Total speed over the last 60 s (oldest first), so a freshly opened window can draw its trace.
    pub fn speed_history(&self) -> Vec<u64> {
        self.inner.history.lock().iter().copied().collect()
    }

    pub fn set_settings(&self, s: Settings) -> Result<(), String> {
        self.inner.engine.global_limiter().set_rate(s.speed_limit);
        self.inner.torrents.update_settings(torrent_jobs::torrent_settings(&s));
        self.inner.db.lock().save_settings(&s).map_err(|e| e.to_string())?;
        *self.inner.settings.write() = s;
        self.inner.bandwidth.lock().reapply();
        self.inner.balance_bandwidth();
        self.inner.schedule();
        Ok(())
    }

    /// The torrent upload cap in force and the upload speed it's based on.
    pub fn upload_status(&self) -> UploadStatus {
        self.inner.upload_status()
    }

    /// The public details of a Spotify / Amazon Music share link (see `music`).
    pub async fn music_lookup(&self, url: &str) -> Result<crate::music::MusicInfo, String> {
        let link = crate::music::parse_link(url).ok_or_else(|| "Not a Spotify or Amazon Music link".to_string())?;
        Ok(crate::music::lookup(&self.inner.engine, &link).await)
    }

    /// A store's search address for this query (stores and their addresses are fixed in `music`).
    pub fn store_search_url(&self, store_id: &str, query: &str) -> Option<String> {
        crate::music::store_url(store_id, query, crate::music::qobuz_region(&crate::music::system_locale()))
    }

    pub async fn probe(&self, url: String, headers: Vec<(String, String)>) -> Result<ProbeInfo, String> {
        let req = DownloadRequest { url, headers, save_dir: PathBuf::new(), file_name: None, connections: 1, allow_html: true };
        self.inner.engine.probe(&req).await.map_err(|e| e.to_string())
    }

    pub fn add(&self, req: AddRequest) -> Result<ItemView, String> {
        let url = req.url.trim().to_string();
        let parsed = url::Url::parse(&url).map_err(|_| format!("Not a valid address: {url}"))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err("Only http and https addresses are supported for now".into());
        }
        let name_fixed = req.file_name.as_deref().is_some_and(|n| !n.trim().is_empty());
        let file_name = sanitize_file_name(
            &req.file_name.filter(|n| !n.trim().is_empty()).or_else(|| file_name_from_url(&url)).unwrap_or_else(|| "download".into()),
        );
        let category = categorize(&file_name).to_string();
        let settings = self.settings();
        let save_dir = req.save_dir.filter(|d| !d.trim().is_empty()).unwrap_or_else(|| {
            let base = PathBuf::from(&settings.download_dir);
            match (settings.category_folders, folder_for(&category)) {
                (true, Some(f)) => base.join(f).to_string_lossy().into_owned(),
                _ => base.to_string_lossy().into_owned(),
            }
        });
        let view = {
            let mut st = self.inner.state.lock();
            let position = st.next_position;
            st.next_position += 1;
            let item = Item {
                id: format!("{:x}{:04x}", now_ms(), fastrand::u16(..)),
                url,
                file_name,
                save_dir,
                file_path: None,
                size: None,
                downloaded: 0,
                status: if req.start { Status::Queued } else { Status::Paused },
                category,
                added_at: now_ms(),
                finished_at: None,
                error: None,
                connections: req.connections.unwrap_or(settings.connections).clamp(1, 32),
                resumable: false,
                headers: req.headers,
                resume: None,
                position,
                name_fixed,
                allow_html: req.allow_page,
                kind: Kind::Http,
                video: None,
                torrent: None,
                convert: None,
            };
            let _ = self.inner.db.lock().save_item(&item);
            let view = ItemView::from(&item);
            st.items.insert(item.id.clone(), item);
            view
        };
        (self.inner.emit)(CoreEvent::Upsert { item: view.clone() });
        self.inner.schedule();
        Ok(view)
    }

    // ---- Video and music links -------------------------------------------------------

    pub fn tools_status(&self) -> ToolsStatus {
        self.inner.tools.status()
    }

    /// Download the video helpers (only ever called after the user agreed in the UI).
    pub async fn install_tools(&self) -> Result<(), String> {
        self.inner.tools.install(&self.inner.engine, false).await
    }

    /// Fetch a newer yt-dlp if there is one. Returns whether it changed.
    pub async fn update_tools(&self) -> Result<bool, String> {
        self.inner.tools.update(&self.inner.engine).await
    }

    /// Read a video page and list its quality choices.
    pub async fn video_info(&self, url: String) -> Result<VideoInfo, String> {
        if !self.inner.tools.ready() {
            return Err("Video tools are not installed".into());
        }
        let tools = self.inner.tools.clone();
        let engine = self.inner.engine.clone();
        tokio::spawn(async move { tools.update_if_stale(&engine).await });
        video::resolve(&self.inner.tools, url.trim()).await
    }

    /// Add a video (or, with `playlist`, every video of its playlist) with the chosen quality.
    pub async fn add_video(&self, req: AddVideoRequest) -> Result<Vec<ItemView>, String> {
        let url = req.url.trim().to_string();
        url::Url::parse(&url).map_err(|_| format!("Not a valid address: {url}"))?;
        let entries = if req.playlist {
            video::playlist_entries(&self.inner.tools, &url).await?
        } else {
            vec![video::Entry { url: url.clone(), title: req.title.clone(), duration: req.duration, thumbnail: req.thumbnail.clone() }]
        };
        if entries.is_empty() {
            return Err("The playlist has no videos".into());
        }
        let audio = matches!(req.option.choice, video::Choice::Audio { .. });
        let category = if audio { "music" } else { "video" }.to_string();
        let settings = self.settings();
        let save_dir = req.save_dir.clone().filter(|d| !d.trim().is_empty()).unwrap_or_else(|| {
            let base = PathBuf::from(&settings.download_dir);
            match (settings.category_folders, folder_for(&category)) {
                (true, Some(f)) => base.join(f).to_string_lossy().into_owned(),
                _ => base.to_string_lossy().into_owned(),
            }
        });
        let mut views = Vec::with_capacity(entries.len());
        for e in entries {
            // One video: the exact formats picked. A playlist: the same choice, per video.
            let selector = if req.playlist { video::generic_selector(&req.option.choice) } else { req.option.selector.clone() };
            let spec = VideoSpec {
                choice: req.option.choice.clone(),
                selector,
                ext: req.option.ext.clone(),
                quality: req.option.label.clone(),
                title: e.title.clone(),
                channel: req.channel.clone(),
                duration: e.duration,
                site: req.site.clone(),
                thumb: None,
                partials: vec![],
                retries: 0,
            };
            let view = {
                let mut st = self.inner.state.lock();
                let position = st.next_position;
                st.next_position += 1;
                let item = Item {
                    id: format!("{:x}{:04x}", now_ms(), fastrand::u16(..)),
                    url: e.url.clone(),
                    file_name: format!("{}.{}", sanitize_file_name(&e.title), spec.ext),
                    save_dir: save_dir.clone(),
                    file_path: None,
                    size: if req.playlist { None } else { req.option.size },
                    downloaded: 0,
                    status: if req.start { Status::Queued } else { Status::Paused },
                    category: category.clone(),
                    added_at: now_ms(),
                    finished_at: None,
                    error: None,
                    connections: 1,
                    resumable: true,
                    headers: vec![],
                    resume: None,
                    position,
                    name_fixed: false,
                    allow_html: false,
                    kind: Kind::Video,
                    video: Some(spec),
                    torrent: None,
                    convert: None,
                };
                let _ = self.inner.db.lock().save_item(&item);
                let view = ItemView::from(&item);
                st.items.insert(item.id.clone(), item);
                view
            };
            (self.inner.emit)(CoreEvent::Upsert { item: view.clone() });
            if let Some(t) = e.thumbnail {
                let inner = self.inner.clone();
                let id = view.id.clone();
                tokio::spawn(async move { inner.fetch_thumbnail(&id, &t).await });
            }
            views.push(view);
        }
        self.inner.schedule();
        Ok(views)
    }

    /// The stored thumbnail of a video item, as a data: URI (the page's CSP allows only those).
    pub fn thumbnail(&self, id: &str) -> Option<String> {
        use base64::Engine as _;
        let name = self.inner.state.lock().items.get(id)?.video.as_ref()?.thumb.clone()?;
        let bytes = std::fs::read(self.inner.data_dir.join("thumbs").join(&name)).ok()?;
        let mime = if name.ends_with(".png") { "image/png" } else if name.ends_with(".webp") { "image/webp" } else { "image/jpeg" };
        Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
    }

    pub fn pause(&self, ids: &[String]) {
        let mut changed = Vec::new();
        {
            let mut st = self.inner.state.lock();
            for id in ids {
                if self.inner.pause_convert(&st, id) {
                    continue; // becomes Paused when the batch stops
                }
                if let Some(r) = st.running.get(id) {
                    r.job.pause(); // becomes Paused when the job stops
                } else if let Some(item) = st.items.get_mut(id) {
                    if item.kind == Kind::Torrent {
                        if self.inner.pause_torrent(item) {
                            changed.push(item.clone());
                        }
                    } else if item.status == Status::Queued {
                        item.status = Status::Paused;
                        changed.push(item.clone());
                    }
                }
            }
        }
        self.inner.persist_and_emit(&changed);
    }

    pub fn resume(&self, ids: &[String]) {
        let mut changed = Vec::new();
        {
            let mut st = self.inner.state.lock();
            for id in ids {
                if let Some(item) = st.items.get_mut(id) {
                    if item.kind == Kind::Torrent {
                        if self.inner.resume_torrent(item) {
                            changed.push(item.clone());
                        }
                        continue;
                    }
                    if matches!(item.status, Status::Paused | Status::Failed) {
                        item.status = Status::Queued;
                        item.error = None;
                        if let Some(v) = item.video.as_mut() {
                            v.retries = 0;
                        }
                        changed.push(item.clone());
                    }
                }
            }
        }
        self.inner.persist_and_emit(&changed);
        self.inner.schedule();
    }

    pub fn pause_all(&self) {
        let ids: Vec<String> = self.inner.state.lock().items.keys().cloned().collect();
        self.pause(&ids);
    }

    pub fn resume_all(&self) {
        let ids: Vec<String> = self.inner.state.lock().items.keys().cloned().collect();
        self.resume(&ids);
    }

    /// Throw away partial data and start from byte 0.
    pub fn restart(&self, ids: &[String]) {
        let mut changed = Vec::new();
        {
            let mut st = self.inner.state.lock();
            for id in ids {
                if st.running.contains_key(id) || st.converting.contains_key(id) {
                    continue;
                }
                if let Some(item) = st.items.get_mut(id) {
                    if item.kind == Kind::Torrent {
                        continue; // a torrent re-checks its own pieces; nothing to start over
                    }
                    if item.kind == Kind::Convert {
                        Inner::restart_convert(item);
                        item.downloaded = 0;
                        item.error = None;
                        item.file_path = None;
                        item.finished_at = None;
                        item.status = Status::Queued;
                        changed.push(item.clone());
                        continue;
                    }
                    if let Some(r) = item.resume.take() {
                        let _ = std::fs::remove_file(part_path(&r.file_path));
                    }
                    if let Some(v) = item.video.as_mut() {
                        v.partials.clear();
                        v.retries = 0;
                        let _ = std::fs::remove_dir_all(video_work_dir(&item.save_dir, &item.id));
                    }
                    item.downloaded = 0;
                    item.size = None;
                    item.error = None;
                    item.file_path = None;
                    item.finished_at = None;
                    item.status = Status::Queued;
                    changed.push(item.clone());
                }
            }
        }
        self.inner.persist_and_emit(&changed);
        self.inner.schedule();
    }

    /// Remove from the list. Unfinished data is always deleted; finished files only if asked.
    pub fn remove(&self, ids: &[String], delete_files: bool) {
        let mut removed = Vec::new();
        {
            let mut st = self.inner.state.lock();
            for id in ids {
                let running = st.running.remove(id);
                let converting = st.converting.remove(id);
                let Some(item) = st.items.remove(id) else { continue };
                if item.kind == Kind::Convert {
                    self.inner.remove_convert(converting, &item, delete_files);
                    removed.push(id.clone());
                    continue;
                }
                if item.kind == Kind::Torrent {
                    self.inner.remove_torrent(&item, delete_files);
                    removed.push(id.clone());
                    continue;
                }
                let part = item.resume.as_ref().map(|r| part_path(&r.file_path));
                let finished = (item.status == Status::Done).then(|| item.file_path.clone()).flatten();
                let work = item.video.as_ref().map(|_| video_work_dir(&item.save_dir, &item.id));
                let jobs_dir = Path::new(&item.save_dir).join(WORK_DIR);
                match running.map(|r| r.job) {
                    Some(Job::Http(h)) => {
                        h.cancel();
                        tokio::spawn(async move {
                            let _ = h.wait().await;
                            if let Some(path) = h.snapshot().file_path {
                                let _ = tokio::fs::remove_file(part_path(&path)).await;
                            }
                        });
                    }
                    Some(Job::Video(v)) => {
                        v.cancel();
                        tokio::spawn(async move {
                            let _ = v.wait().await;
                            if let Some(w) = work {
                                let _ = tokio::fs::remove_dir_all(w).await;
                            }
                            remove_if_empty(&jobs_dir);
                        });
                    }
                    None => {
                        if let Some(p) = part {
                            let _ = std::fs::remove_file(p);
                        }
                        if let Some(w) = work {
                            let _ = std::fs::remove_dir_all(w);
                            remove_if_empty(&jobs_dir);
                        }
                    }
                }
                if delete_files {
                    if let Some(f) = finished {
                        let _ = std::fs::remove_file(f);
                    }
                }
                removed.push(id.clone());
            }
        }
        let _ = self.inner.db.lock().delete_items(&removed);
        (self.inner.emit)(CoreEvent::Removed { ids: removed });
        self.inner.schedule();
    }

    pub fn get(&self, id: &str) -> Option<ItemView> {
        self.inner.state.lock().items.get(id).map(ItemView::from)
    }

    /// Save everything (call before the app exits).
    pub async fn shutdown(&self) {
        self.inner.quitting.store(true, Ordering::SeqCst);
        // Stop conversions (ffmpeg and its half-written file go); they resume at the next start.
        let jobs: Vec<crate::convert::ConvertJob> = self.inner.state.lock().converting.values().cloned().collect();
        for j in &jobs {
            j.pause();
        }
        for j in &jobs {
            let _ = tokio::time::timeout(Duration::from_secs(5), j.wait()).await;
        }
        self.inner.checkpoint().await;
        self.inner.torrents.shutdown().await;
    }
}

impl Inner {
    fn persist_and_emit(&self, items: &[Item]) {
        if items.is_empty() {
            return;
        }
        let _ = self.db.lock().save_items(items.iter());
        for i in items {
            (self.emit)(CoreEvent::Upsert { item: ItemView::from(i) });
        }
    }

    /// Start queued downloads up to the concurrency limit, in queue order.
    fn schedule(self: &Arc<Self>) {
        self.schedule_converts();
        let max = self.settings.read().max_concurrent.max(1);
        let mut started = Vec::new();
        {
            let mut st = self.state.lock();
            if st.running.len() >= max {
                return;
            }
            let mut queued: Vec<(i64, String)> = st
                .items
                .values()
                .filter(|i| i.status == Status::Queued && !st.running.contains_key(&i.id))
                .map(|i| (i.position, i.id.clone()))
                .collect();
            queued.sort();
            for (_, id) in queued {
                if st.running.len() >= max {
                    break;
                }
                let item = st.items.get_mut(&id).unwrap();
                if item.kind == Kind::Convert {
                    continue; // conversions run on their own (`schedule_converts`)
                }
                if item.kind == Kind::Torrent {
                    // Never the HTTP engine's job (older versions could leave one Queued).
                    item.status = Status::Downloading;
                    started.push(item.clone());
                    continue;
                }
                if item.kind == Kind::Video {
                    let Some(spec) = item.video.clone() else { continue };
                    if !self.tools.ready() {
                        item.status = Status::Failed;
                        item.error = Some("Video tools are not installed; add a video link to get them".into());
                        started.push(item.clone());
                        continue;
                    }
                    // yt-dlp works in a folder of its own: it skips (and with -x even deletes) any
                    // file that already has the name it wants, so it never sees the user's files.
                    let work = video_work_dir(&item.save_dir, &item.id);
                    let _ = std::fs::create_dir_all(&work);
                    hide(&Path::new(&item.save_dir).join(WORK_DIR));
                    let args = video::download_args(&self.tools, &spec, &item.url, &work);
                    let job = VideoJob::start(self.tools.yt_dlp(), args);
                    item.status = Status::Downloading;
                    item.error = None;
                    started.push(item.clone());
                    let (j, me, id2) = (job.clone(), self.clone(), id.clone());
                    tokio::spawn(async move {
                        let result = j.wait().await;
                        me.finished_video(&id2, &j, result);
                    });
                    let downloaded = item.downloaded;
                    st.running.insert(
                        id,
                        Running { job: Job::Video(job), last_bytes: downloaded, last_at: Instant::now(), speed: 0.0, parts: HashMap::new() },
                    );
                    continue;
                }
                let req = DownloadRequest {
                    url: item.url.clone(),
                    headers: item.headers.clone(),
                    save_dir: PathBuf::from(&item.save_dir),
                    file_name: item.name_fixed.then(|| item.file_name.clone()),
                    connections: item.connections as usize,
                    allow_html: item.allow_html,
                };
                let handle = self.engine.start(req, item.resume.clone(), vec![]);
                item.status = Status::Downloading;
                item.error = None;
                started.push(item.clone());
                let h = handle.clone();
                let me = self.clone();
                let id2 = id.clone();
                tokio::spawn(async move {
                    let result = h.wait().await;
                    me.finished(&id2, &h, result);
                });
                let downloaded = item.downloaded;
                st.running.insert(
                    id,
                    Running { job: Job::Http(handle), last_bytes: downloaded, last_at: Instant::now(), speed: 0.0, parts: HashMap::new() },
                );
            }
        }
        self.persist_and_emit(&started);
    }

    fn finished(self: &Arc<Self>, id: &str, handle: &DownloadHandle, result: Result<das_engine::Completed, EngineError>) {
        let (item, completed) = {
            let mut st = self.state.lock();
            st.running.remove(id);
            let Some(item) = st.items.get_mut(id) else { return }; // removed meanwhile
            let snap = handle.snapshot();
            item.resumable = snap.resumable;
            if let Some(size) = snap.size {
                item.size = Some(size);
            }
            item.downloaded = snap.downloaded;
            let mut completed = false;
            match result {
                Ok(done) => {
                    item.status = Status::Done;
                    item.file_path = Some(done.path.to_string_lossy().into_owned());
                    item.file_name = done.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    item.size = Some(done.size);
                    item.downloaded = done.size;
                    item.finished_at = Some(now_ms());
                    item.resume = None;
                    item.error = None;
                    completed = true;
                }
                Err(EngineError::Paused) => {
                    item.status = Status::Paused;
                    item.resume = handle.resume_state();
                }
                Err(EngineError::Cancelled) => {
                    item.status = Status::Paused;
                }
                Err(e) => {
                    item.status = Status::Failed;
                    item.error = Some(friendly_error(&e));
                    item.resume = handle.resume_state();
                }
            }
            (item.clone(), completed)
        };
        let _ = self.db.lock().save_item(&item);
        let view = ItemView::from(&item);
        (self.emit)(CoreEvent::Upsert { item: view.clone() });
        if completed {
            (self.emit)(CoreEvent::Completed { item: view });
        }
        self.schedule();
    }

    fn finished_video(self: &Arc<Self>, id: &str, job: &VideoJob, result: Result<PathBuf, VideoError>) {
        let (item, completed) = {
            let mut st = self.state.lock();
            st.running.remove(id);
            let Some(item) = st.items.get_mut(id) else { return };
            let vs = job.state();
            let mut completed = false;
            match result {
                Ok(made) => {
                    let path = place_video(&item.save_dir, &item.id, &made);
                    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(vs.known_total());
                    item.status = Status::Done;
                    item.file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    item.file_path = Some(path.to_string_lossy().into_owned());
                    item.size = Some(size);
                    item.downloaded = size;
                    item.finished_at = Some(now_ms());
                    item.error = None;
                    if let Some(v) = item.video.as_mut() {
                        v.partials.clear();
                        v.retries = 0;
                    }
                    completed = true;
                }
                Err(e) => {
                    if !vs.streams.is_empty() {
                        item.downloaded = vs.downloaded();
                    }
                    if let Some(v) = item.video.as_mut() {
                        for p in vs.partials {
                            if !v.partials.contains(&p) {
                                v.partials.push(p);
                            }
                        }
                    }
                    match e {
                        VideoError::Paused | VideoError::Cancelled => item.status = Status::Paused,
                        VideoError::Failed(msg) => {
                            // Try again by itself a couple of times; finished streams are kept.
                            let v = item.video.as_mut();
                            match v {
                                Some(v) if video::is_transient(&msg) && v.retries < 2 => {
                                    v.retries += 1;
                                    item.status = Status::Queued;
                                }
                                _ => {
                                    item.status = Status::Failed;
                                    item.error = Some(msg);
                                }
                            }
                        }
                    }
                }
            }
            (item.clone(), completed)
        };
        let _ = self.db.lock().save_item(&item);
        let view = ItemView::from(&item);
        (self.emit)(CoreEvent::Upsert { item: view.clone() });
        if completed {
            (self.emit)(CoreEvent::Completed { item: view });
        }
        self.schedule();
    }

    /// Store a small copy of a video's thumbnail for the list and the pass.
    async fn fetch_thumbnail(&self, id: &str, url: &str) {
        let Ok((bytes, ct)) = self.engine.fetch_small(url, 1 << 20).await else { return };
        let ext = match ct.as_deref().unwrap_or("") {
            c if c.contains("png") => "png",
            c if c.contains("webp") => "webp",
            _ => "jpg",
        };
        let dir = self.data_dir.join("thumbs");
        let name = format!("{id}.{ext}");
        if std::fs::create_dir_all(&dir).is_err() || std::fs::write(dir.join(&name), bytes).is_err() {
            return;
        }
        let item = {
            let mut st = self.state.lock();
            let Some(item) = st.items.get_mut(id) else { return };
            let Some(v) = item.video.as_mut() else { return };
            v.thumb = Some(name);
            item.clone()
        };
        self.persist_and_emit(&[item]);
    }

    /// Update live numbers; returns whether anything is running.
    fn progress_tick(&self) -> bool {
        let visible = self.visible.load(Ordering::Relaxed);
        let mut renamed = Vec::new();
        let (mut tchanged, mut tcompleted) = (Vec::new(), Vec::new());
        let (items, totals) = {
            let mut st = self.state.lock();
            if st.running.is_empty() && st.converting.is_empty() && !Inner::has_active_torrents(&st) {
                // Send one final all-zero update so the board doesn't keep showing the last speed.
                if self.was_running.swap(false, Ordering::Relaxed) {
                    let queued = st.items.values().filter(|i| i.status == Status::Queued).count();
                    drop(st);
                    self.push_history(0);
                    (self.emit)(CoreEvent::Progress { items: vec![], totals: Totals { queued, ..Totals::default() } });
                }
                return false;
            }
            self.was_running.store(true, Ordering::Relaxed);
            let now = Instant::now();
            let mut out = Vec::with_capacity(st.running.len());
            let mut totals = Totals::default();
            let State { items, running, converting, .. } = &mut *st;
            self.convert_progress(items, converting, &mut out);
            for (id, r) in running.iter_mut() {
                let snap = match &r.job {
                    Job::Http(h) => h.snapshot(),
                    Job::Video(v) => {
                        let vs = v.state();
                        let Some(item) = items.get_mut(id) else { continue };
                        // A resumed job reads the page again first; keep the old figure until it reports.
                        if !vs.streams.is_empty() {
                            item.downloaded = vs.downloaded();
                        }
                        let total = item.size.unwrap_or(0).max(vs.known_total());
                        if total > 0 {
                            item.size = Some(total);
                        }
                        let speed = vs.speed.round() as u64;
                        let left = item.size.map(|s| s.saturating_sub(item.downloaded));
                        totals.speed += speed;
                        totals.active += 1;
                        totals.remaining_bytes += left.unwrap_or(0);
                        out.push(Progress {
                            id: id.clone(),
                            downloaded: item.downloaded,
                            size: item.size,
                            speed,
                            eta: left.filter(|_| speed > 0).map(|l| l / speed),
                            active_connections: 1,
                            gaps: vec![],
                            heads: vec![],
                            parts: vec![],
                            parts_total: 0,
                            file_name: item.file_name.clone(),
                            phase: Some(vs.phase),
                            torrent: None,
                        });
                        continue;
                    }
                };
                let dt = now.duration_since(r.last_at).as_secs_f64().max(0.001);
                let inst = snap.downloaded.saturating_sub(r.last_bytes) as f64 / dt;
                r.speed = if r.speed == 0.0 { inst } else { r.speed * 0.7 + inst * 0.3 };
                // Live parts: one per connection that is writing right now.
                let mut parts = Vec::new();
                let mut seen = std::collections::HashSet::new();
                for s in snap.segments.iter().filter(|s| s.active && s.pos < s.end && s.end != u64::MAX) {
                    seen.insert(s.start);
                    let entry = r.parts.entry(s.start).or_insert((s.pos, 0.0));
                    let inst = s.pos.saturating_sub(entry.0) as f64 / dt;
                    entry.1 = if entry.1 == 0.0 { inst } else { entry.1 * 0.6 + inst * 0.4 };
                    entry.0 = s.pos;
                    parts.push(PartView { start: s.start, pos: s.pos, end: s.end, speed: entry.1.round() as u64 });
                }
                r.parts.retain(|k, _| seen.contains(k));
                let parts_total = snap.segments.len();
                r.last_bytes = snap.downloaded;
                r.last_at = now;
                let Some(item) = items.get_mut(id) else { continue };
                item.downloaded = snap.downloaded;
                if snap.size.is_some() {
                    item.size = snap.size;
                }
                item.resumable = snap.resumable;
                if let Some(path) = &snap.file_path {
                    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    if !name.is_empty() && name != item.file_name {
                        item.file_name = name;
                        item.category = categorize_with(&item.file_name, snap.content_type.as_deref()).to_string();
                        renamed.push(ItemView::from(&*item));
                    }
                }
                // A name without a telling extension: let the server's Content-Type decide.
                if item.category == "other" {
                    let by_type = categorize_with(&item.file_name, snap.content_type.as_deref());
                    if by_type != "other" {
                        item.category = by_type.to_string();
                        renamed.push(ItemView::from(&*item));
                    }
                }
                let speed = r.speed.round() as u64;
                let left = item.size.map(|s| s.saturating_sub(item.downloaded));
                let eta = match (left, speed) {
                    (Some(l), s) if s > 0 => Some(l / s),
                    _ => None,
                };
                totals.speed += speed;
                totals.active += 1;
                totals.remaining_bytes += left.unwrap_or(0);
                let (gaps, heads) = segment_map(&snap);
                out.push(Progress {
                    id: id.clone(),
                    downloaded: item.downloaded,
                    size: item.size,
                    speed,
                    eta,
                    active_connections: snap.connections,
                    gaps,
                    heads,
                    parts,
                    parts_total,
                    file_name: item.file_name.clone(),
                    phase: None,
                    torrent: None,
                });
            }
            self.torrent_progress(items, visible, &mut out, &mut totals, &mut tchanged, &mut tcompleted);
            for item in items.values().filter(|i| i.status == Status::Queued) {
                totals.queued += 1;
                totals.remaining_bytes += item.size.map(|s| s.saturating_sub(item.downloaded)).unwrap_or(0);
            }
            if totals.speed > 0 {
                totals.eta = Some(totals.remaining_bytes / totals.speed);
            }
            (out, totals)
        };
        self.push_history(totals.speed);
        self.persist_and_emit(&tchanged);
        for c in tcompleted {
            (self.emit)(CoreEvent::Completed { item: ItemView::from(&c) });
        }
        for v in renamed {
            (self.emit)(CoreEvent::Upsert { item: v });
        }
        if visible {
            (self.emit)(CoreEvent::Progress { items, totals });
        }
        true
    }

    fn push_history(&self, speed: u64) {
        let mut h = self.history.lock();
        if h.len() == HISTORY {
            h.pop_front();
        }
        h.push_back(speed);
    }

    /// Persist resume points: snapshot, fsync data, then write the snapshot.
    async fn checkpoint(&self) {
        let handles: Vec<(String, DownloadHandle)> = self
            .state
            .lock()
            .running
            .iter()
            .filter_map(|(id, r)| match &r.job {
                Job::Http(h) => Some((id.clone(), h.clone())),
                Job::Video(_) => None,
            })
            .collect();
        let mut states = Vec::with_capacity(handles.len());
        for (id, h) in handles {
            if let Some(s) = h.checkpoint().await {
                states.push((id, s));
            }
        }
        let items: Vec<Item> = {
            let mut st = self.state.lock();
            for (id, s) in states {
                if let Some(item) = st.items.get_mut(&id) {
                    if item.status == Status::Downloading {
                        item.resume = Some(s);
                    }
                }
            }
            st.items.values().filter(|i| i.status == Status::Downloading).cloned().collect()
        };
        let _ = self.db.lock().save_items(items.iter());
    }
}

/// Where unfinished video jobs keep their pieces: `<save dir>\.dasidm\<id>`.
const WORK_DIR: &str = ".dasidm";

fn video_work_dir(save_dir: &str, id: &str) -> PathBuf {
    Path::new(save_dir).join(WORK_DIR).join(id)
}

/// The finished media file in a video job's work folder: the biggest file that is not a
/// partial download, a resume note or a leftover thumbnail.
fn finished_media(work: &Path) -> Option<PathBuf> {
    let skip = [".part", ".ytdl", ".temp", ".tmp", ".jpg", ".jpeg", ".png", ".webp"];
    std::fs::read_dir(work)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter(|e| {
            let n = e.file_name().to_string_lossy().to_ascii_lowercase();
            !skip.iter().any(|s| n.ends_with(s)) && !n.contains(".part-frag")
        })
        .max_by_key(|e| e.metadata().map(|m| m.len()).unwrap_or(0))
        .map(|e| e.path())
}

/// Move a finished video from its work folder next to the user's files, under a free name, and
/// clean the work folder up. Returns where the file is now.
///
/// The path yt-dlp reports is a hint only: characters it substitutes in file names (a title's
/// `|` becomes `｜`) can be lost on the way back, so the real file is looked up in the folder.
fn place_video(save_dir: &str, id: &str, reported: &Path) -> PathBuf {
    let work = video_work_dir(save_dir, id);
    let made = if reported.is_file() { reported.to_path_buf() } else { finished_media(&work).unwrap_or_else(|| reported.to_path_buf()) };
    if !made.is_file() {
        return made;
    }
    let name = made.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "video".into());
    let dest = unique_path(Path::new(save_dir), &name);
    let moved = std::fs::rename(&made, &dest).is_ok()
        // Locked for a moment (a virus scan) or another drive: copy, then drop the original.
        || (std::fs::copy(&made, &dest).is_ok() && std::fs::remove_file(&made).map(|_| true).unwrap_or(true));
    if !moved {
        return made; // still usable where it is; never lost
    }
    let _ = std::fs::remove_dir_all(&work);
    remove_if_empty(&Path::new(save_dir).join(WORK_DIR));
    dest
}

/// Mark a folder hidden in Explorer.
fn hide(dir: &Path) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_HIDDEN, GetFileAttributesW, INVALID_FILE_ATTRIBUTES, SetFileAttributesW};
    let wide: Vec<u16> = dir.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        let attrs = GetFileAttributesW(wide.as_ptr());
        if attrs != INVALID_FILE_ATTRIBUTES && attrs & FILE_ATTRIBUTE_HIDDEN == 0 {
            SetFileAttributesW(wide.as_ptr(), attrs | FILE_ATTRIBUTE_HIDDEN);
        }
    }
}

fn remove_if_empty(dir: &Path) {
    let _ = std::fs::remove_dir(dir); // fails (and does nothing) unless empty
}

/// Missing ranges and live connection heads, as fractions of the file.
fn segment_map(snap: &das_engine::Snapshot) -> (Vec<[f32; 2]>, Vec<f32>) {
    let Some(size) = snap.size.filter(|s| *s > 0) else { return (vec![], vec![]) };
    let f = |x: u64| (x.min(size) as f64 / size as f64) as f32;
    let mut gaps: Vec<[f32; 2]> = snap.segments.iter().filter(|s| s.pos < s.end).map(|s| [f(s.pos), f(s.end)]).collect();
    if gaps.len() > 96 {
        gaps.sort_by(|a, b| (b[1] - b[0]).total_cmp(&(a[1] - a[0])));
        gaps.truncate(96);
        gaps.sort_by(|a, b| a[0].total_cmp(&b[0]));
    }
    let heads = snap.segments.iter().filter(|s| s.active && s.pos < s.end).map(|s| f(s.pos)).collect();
    (gaps, heads)
}

fn friendly_error(e: &EngineError) -> String {
    match e {
        EngineError::Http(404) | EngineError::Http(410) => "File not found on the server".into(),
        EngineError::Http(401) => "The server wants a login".into(),
        EngineError::Http(403) => "Link expired or access denied".into(),
        EngineError::Http(416) => "Server refused the byte range".into(),
        EngineError::Http(code) => format!("Server error {code}"),
        EngineError::ValidatorMismatch => "File changed on the server; restart it".into(),
        EngineError::WebPage => "This link is a web page, not a file".into(),
        EngineError::NoProgress => "Server stopped sending data".into(),
        EngineError::Network(m) => format!("Network problem: {m}"),
        EngineError::Io(m) => format!("Disk problem: {m}"),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finished_video_is_moved_even_when_its_reported_name_is_wrong() {
        let tmp = tempfile::tempdir().unwrap();
        let save = tmp.path().to_string_lossy().into_owned();
        let work = video_work_dir(&save, "job1");
        std::fs::create_dir_all(&work).unwrap();
        // What yt-dlp wrote (with the full-width bar) and leftovers beside it.
        let real = work.join("Trailer ｜ Cast.mkv");
        std::fs::write(&real, vec![1u8; 4096]).unwrap();
        std::fs::write(work.join("Trailer ｜ Cast.f251.webm.part"), vec![0u8; 8192]).unwrap();
        std::fs::write(work.join("Trailer ｜ Cast.webp"), vec![0u8; 100]).unwrap();
        // A file with the same name already in the folder must not be overwritten.
        std::fs::write(tmp.path().join("Trailer ｜ Cast.mkv"), b"mine").unwrap();

        // The path as an older yt-dlp output decoding reported it: the bar is gone.
        let reported = work.join("Trailer  Cast.mkv");
        let placed = place_video(&save, "job1", &reported);

        assert_eq!(placed, tmp.path().join("Trailer ｜ Cast (1).mkv"));
        assert_eq!(std::fs::read(&placed).unwrap().len(), 4096);
        assert_eq!(std::fs::read(tmp.path().join("Trailer ｜ Cast.mkv")).unwrap(), b"mine");
        assert!(!work.exists(), "the work folder is cleaned up");
        assert!(!tmp.path().join(WORK_DIR).exists());
    }
}
