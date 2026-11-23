//! Owns all downloads: queue admission, engine jobs, progress batching, checkpoints.

use crate::categories::{categorize, categorize_with, folder_for};
use crate::db::Db;
use crate::model::*;
use crate::settings::Settings;
use das_engine::filename::{file_name_from_url, part_path, sanitize_file_name};
use das_engine::{DownloadHandle, DownloadRequest, Engine, EngineConfig, EngineError, ProbeInfo};
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

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
}

const HISTORY: usize = 240;

#[derive(Default)]
struct State {
    items: HashMap<String, Item>,
    running: HashMap<String, Running>,
    next_position: i64,
}

struct Running {
    handle: DownloadHandle,
    last_bytes: u64,
    last_at: Instant,
    speed: f64,
    /// Per-part (keyed by the part's start byte): last written position and smoothed speed.
    parts: HashMap<u64, (u64, f64)>,
}

impl Manager {
    pub fn open(data_dir: &Path, emit: Emit) -> Result<Self, String> {
        let db = Db::open(&data_dir.join("dasidm.db")).map_err(|e| e.to_string())?;
        let settings = db.load_settings();
        let mut state = State::default();
        for mut item in db.load_items().map_err(|e| e.to_string())? {
            // A crash or exit mid-download: pick it up again.
            if item.status == Status::Downloading {
                item.status = Status::Queued;
            }
            state.next_position = state.next_position.max(item.position + 1);
            state.items.insert(item.id.clone(), item);
        }
        let engine = Engine::new(EngineConfig::default());
        engine.global_limiter().set_rate(settings.speed_limit);
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
            }),
        })
    }

    /// Background loop: progress at 4 Hz (only while visible), checkpoints every 2 s.
    /// Must run on a tokio runtime; it also starts queued downloads.
    pub async fn run(self) {
        self.inner.schedule();
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
        self.inner.db.lock().save_settings(&s).map_err(|e| e.to_string())?;
        *self.inner.settings.write() = s;
        self.inner.schedule();
        Ok(())
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

    pub fn pause(&self, ids: &[String]) {
        let mut changed = Vec::new();
        {
            let mut st = self.inner.state.lock();
            for id in ids {
                if let Some(r) = st.running.get(id) {
                    r.handle.pause(); // becomes Paused when the job stops
                } else if let Some(item) = st.items.get_mut(id) {
                    if item.status == Status::Queued {
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
                    if matches!(item.status, Status::Paused | Status::Failed) {
                        item.status = Status::Queued;
                        item.error = None;
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
                if st.running.contains_key(id) {
                    continue;
                }
                if let Some(item) = st.items.get_mut(id) {
                    if let Some(r) = item.resume.take() {
                        let _ = std::fs::remove_file(part_path(&r.file_path));
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
                let Some(item) = st.items.remove(id) else { continue };
                let part = item.resume.as_ref().map(|r| part_path(&r.file_path));
                let finished = (item.status == Status::Done).then(|| item.file_path.clone()).flatten();
                match running {
                    Some(r) => {
                        r.handle.cancel();
                        let h = r.handle.clone();
                        tokio::spawn(async move {
                            let _ = h.wait().await;
                            if let Some(path) = h.snapshot().file_path {
                                let _ = tokio::fs::remove_file(part_path(&path)).await;
                            }
                        });
                    }
                    None => {
                        if let Some(p) = part {
                            let _ = std::fs::remove_file(p);
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
        self.inner.checkpoint().await;
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
                    Running { handle, last_bytes: downloaded, last_at: Instant::now(), speed: 0.0, parts: HashMap::new() },
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

    /// Update live numbers; returns whether anything is running.
    fn progress_tick(&self) -> bool {
        let visible = self.visible.load(Ordering::Relaxed);
        let mut renamed = Vec::new();
        let (items, totals) = {
            let mut st = self.state.lock();
            if st.running.is_empty() {
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
            let State { items, running, .. } = &mut *st;
            for (id, r) in running.iter_mut() {
                let snap = r.handle.snapshot();
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
                });
            }
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
        let handles: Vec<(String, DownloadHandle)> =
            self.state.lock().running.iter().map(|(id, r)| (id.clone(), r.handle.clone())).collect();
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
