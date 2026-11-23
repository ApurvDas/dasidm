//! One running download: probe, supervisor, work-stealing workers, finish.

use crate::filename::{self, part_path, unique_path};
use crate::storage;
use crate::{Completed, DownloadRequest, EngineConfig, EngineError, ProbeInfo, RateLimiter, ResumeState};
use bytes::BytesMut;
use futures_util::StreamExt;
use parking_lot::Mutex;
use reqwest::header::{self, HeaderMap, HeaderName, HeaderValue};
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering::*};
use std::time::Duration;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

const OPEN_END: u64 = u64::MAX;
const STOP_NONE: u8 = 0;
const STOP_PAUSE: u8 = 1;
const STOP_CANCEL: u8 = 2;
pub const MAX_CONNECTIONS: usize = 32;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    #[default]
    Connecting,
    Downloading,
    Finishing,
    Done,
    Paused,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Serialize)]
pub struct SegmentView {
    pub start: u64,
    pub pos: u64,
    pub end: u64,
    pub active: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub phase: Phase,
    pub downloaded: u64,
    pub size: Option<u64>,
    pub connections: usize,
    pub file_path: Option<PathBuf>,
    pub final_url: String,
    pub resumable: bool,
    pub segments: Vec<SegmentView>,
    pub error: Option<EngineError>,
    /// The server's Content-Type, once known (used to categorise names without an extension).
    pub content_type: Option<String>,
}

struct Segment {
    start: u64,
    st: Mutex<SegSt>,
    active: AtomicBool,
}

struct SegSt {
    /// Next byte to write; everything before it is on disk.
    pos: u64,
    /// Exclusive. Other workers may shrink it (steal the tail).
    end: u64,
}

impl Segment {
    fn new(start: u64, pos: u64, end: u64, active: bool) -> Arc<Self> {
        Arc::new(Self { start, st: Mutex::new(SegSt { pos, end }), active: AtomicBool::new(active) })
    }
    fn remaining(&self) -> u64 {
        let s = self.st.lock();
        s.end.saturating_sub(s.pos)
    }
}

#[derive(Default)]
struct Meta {
    final_url: String,
    size: Option<u64>,
    etag: Option<String>,
    last_modified: Option<String>,
    file_path: Option<PathBuf>,
    content_type: Option<String>,
    phase: Phase,
    error: Option<EngineError>,
}

struct Shared {
    cfg: Arc<EngineConfig>,
    req: DownloadRequest,
    limiters: Vec<Arc<RateLimiter>>,
    token: CancellationToken,
    stop: AtomicU8,
    resumable: AtomicBool,
    target: AtomicUsize,
    active: AtomicUsize,
    downloaded: AtomicU64,
    meta: Mutex<Meta>,
    segments: Mutex<Vec<Arc<Segment>>>,
    file: Mutex<Option<Arc<File>>>,
    fatal: Mutex<Option<EngineError>>,
}

/// Control and observation of one download. Cheap to clone.
#[derive(Clone)]
pub struct DownloadHandle {
    shared: Arc<Shared>,
    done: watch::Receiver<Option<Result<Completed, EngineError>>>,
}

impl DownloadHandle {
    /// Stop gracefully; `resume_state()` then describes what is left.
    pub fn pause(&self) {
        self.shared.stop.store(STOP_PAUSE, SeqCst);
        self.shared.token.cancel();
    }

    pub fn cancel(&self) {
        self.shared.stop.store(STOP_CANCEL, SeqCst);
        self.shared.token.cancel();
    }

    pub fn set_connections(&self, n: usize) {
        self.shared.target.store(n.clamp(1, MAX_CONNECTIONS), SeqCst);
    }

    pub fn downloaded(&self) -> u64 {
        self.shared.downloaded.load(Relaxed)
    }

    pub fn snapshot(&self) -> Snapshot {
        let s = &self.shared;
        let mut segments: Vec<SegmentView> = s
            .segments
            .lock()
            .iter()
            .map(|seg| {
                let st = seg.st.lock();
                SegmentView { start: seg.start, pos: st.pos, end: st.end, active: seg.active.load(Relaxed) }
            })
            .collect();
        segments.sort_by_key(|v| v.start);
        let m = s.meta.lock();
        Snapshot {
            phase: m.phase,
            downloaded: s.downloaded.load(Relaxed),
            size: m.size,
            connections: s.active.load(Relaxed),
            file_path: m.file_path.clone(),
            final_url: m.final_url.clone(),
            resumable: s.resumable.load(Relaxed),
            segments,
            error: m.error.clone(),
            content_type: m.content_type.clone(),
        }
    }

    pub fn resume_state(&self) -> Option<ResumeState> {
        self.shared.resume_state()
    }

    /// Flush written data to disk, then return a resume state that never claims unwritten bytes.
    pub async fn checkpoint(&self) -> Option<ResumeState> {
        let state = self.shared.resume_state()?;
        let file = self.shared.file.lock().clone();
        if let Some(f) = file {
            let _ = tokio::task::spawn_blocking(move || f.sync_data()).await;
        }
        Some(state)
    }

    pub fn is_finished(&self) -> bool {
        self.done.borrow().is_some()
    }

    pub async fn wait(&self) -> Result<Completed, EngineError> {
        let mut rx = self.done.clone();
        loop {
            if let Some(r) = rx.borrow_and_update().clone() {
                return r;
            }
            if rx.changed().await.is_err() {
                return Err(EngineError::Cancelled);
            }
        }
    }
}

impl Shared {
    fn resume_state(&self) -> Option<ResumeState> {
        let m = self.meta.lock();
        let file_path = m.file_path.clone()?;
        let resumable = self.resumable.load(Relaxed) && m.size.is_some();
        let mut remaining: Vec<(u64, u64)> = if resumable {
            self.segments
                .lock()
                .iter()
                .filter_map(|seg| {
                    let st = seg.st.lock();
                    (st.pos < st.end).then_some((st.pos, st.end))
                })
                .collect()
        } else {
            Vec::new()
        };
        remaining.sort_unstable();
        Some(ResumeState {
            final_url: m.final_url.clone(),
            size: m.size,
            etag: m.etag.clone(),
            last_modified: m.last_modified.clone(),
            file_path,
            resumable,
            remaining,
        })
    }

    fn set_phase(&self, p: Phase) {
        self.meta.lock().phase = p;
    }

    fn set_fatal(&self, e: EngineError) {
        let mut f = self.fatal.lock();
        if f.is_none() {
            *f = Some(e);
        }
        self.token.cancel();
    }

    fn url(&self) -> String {
        let m = self.meta.lock();
        if m.final_url.is_empty() { self.req.url.clone() } else { m.final_url.clone() }
    }

    fn total_remaining(&self) -> u64 {
        self.segments.lock().iter().map(|s| s.remaining()).fold(0u64, |a, b| a.saturating_add(b))
    }
}

pub(crate) fn spawn(
    cfg: Arc<EngineConfig>,
    req: DownloadRequest,
    resume: Option<ResumeState>,
    limiters: Vec<Arc<RateLimiter>>,
) -> DownloadHandle {
    let target = req.connections.clamp(1, MAX_CONNECTIONS);
    let shared = Arc::new(Shared {
        cfg,
        req,
        limiters,
        token: CancellationToken::new(),
        stop: AtomicU8::new(STOP_NONE),
        resumable: AtomicBool::new(false),
        target: AtomicUsize::new(target),
        active: AtomicUsize::new(0),
        downloaded: AtomicU64::new(0),
        meta: Mutex::new(Meta::default()),
        segments: Mutex::new(Vec::new()),
        file: Mutex::new(None),
        fatal: Mutex::new(None),
    });
    let (tx, rx) = watch::channel(None);
    let s = shared.clone();
    tokio::spawn(async move {
        let mut result = run(s.clone(), resume).await;
        if result.is_err() {
            match s.stop.load(SeqCst) {
                STOP_PAUSE if s.fatal.lock().is_none() => result = Err(EngineError::Paused),
                STOP_CANCEL => result = Err(EngineError::Cancelled),
                _ => {}
            }
        }
        {
            let mut m = s.meta.lock();
            m.phase = match &result {
                Ok(_) => Phase::Done,
                Err(EngineError::Paused) => Phase::Paused,
                Err(EngineError::Cancelled) => Phase::Cancelled,
                Err(e) => {
                    m.error = Some(e.clone());
                    Phase::Failed
                }
            };
        }
        // Release the file handle so the part file can be resumed, moved or deleted.
        if !matches!(result, Ok(_)) {
            let file = s.file.lock().take();
            if let Some(f) = file {
                let _ = tokio::task::spawn_blocking(move || f.sync_data()).await;
            }
        }
        let _ = tx.send(Some(result));
    });
    DownloadHandle { shared, done: rx }
}

async fn run(s: Arc<Shared>, resume: Option<ResumeState>) -> Result<Completed, EngineError> {
    s.set_phase(Phase::Connecting);
    url::Url::parse(&s.req.url).map_err(|e| EngineError::BadUrl(e.to_string()))?;
    let usable = resume.filter(|r| {
        r.resumable && r.size.is_some() && part_path(&r.file_path).exists()
    });

    let first: Option<(Arc<Segment>, Option<Response>)>;
    let client0 = make_client(&s.cfg)?;
    if let Some(r) = usable {
        {
            let mut m = s.meta.lock();
            m.final_url = r.final_url.clone();
            m.size = r.size;
            m.etag = r.etag.clone();
            m.last_modified = r.last_modified.clone();
            m.file_path = Some(r.file_path.clone());
        }
        s.resumable.store(true, SeqCst);
        s.downloaded.store(r.downloaded(), SeqCst);
        let part = part_path(&r.file_path);
        let f = tokio::task::spawn_blocking(move || storage::open_part(&part, None, false))
            .await
            .map_err(|e| EngineError::Io(e.to_string()))?
            .map_err(io_err)?;
        *s.file.lock() = Some(Arc::new(f));
        *s.segments.lock() = r.remaining.iter().map(|&(a, b)| Segment::new(a, a, b, false)).collect();
        first = None;
    } else {
        let resp = first_request(&s, &client0).await?;
        let content_type = header_str(resp.headers(), header::CONTENT_TYPE);
        // Refuse a web page before any file exists, unless the user asked for the page itself.
        if !s.req.allow_html && crate::is_web_page(content_type.as_deref()) {
            return Err(EngineError::WebPage);
        }
        s.meta.lock().content_type = content_type;
        let (size, resumable) = parse_size(&resp, 0);
        let name = detect_name(s.req.file_name.as_deref(), &resp);
        let dir = s.req.save_dir.clone();
        let final_path = unique_path(&dir, &name);
        let part = part_path(&final_path);
        {
            let mut m = s.meta.lock();
            m.final_url = resp.url().to_string();
            m.size = size;
            m.etag = header_str(resp.headers(), header::ETAG);
            m.last_modified = header_str(resp.headers(), header::LAST_MODIFIED);
            m.file_path = Some(final_path);
        }
        s.resumable.store(resumable && size.is_some(), SeqCst);
        let f = tokio::task::spawn_blocking(move || storage::open_part(&part, size, true))
            .await
            .map_err(|e| EngineError::Io(e.to_string()))?
            .map_err(io_err)?;
        *s.file.lock() = Some(Arc::new(f));
        let seg = Segment::new(0, 0, size.unwrap_or(OPEN_END), true);
        s.segments.lock().push(seg.clone());
        first = Some((seg, Some(resp)));
    }

    s.set_phase(Phase::Downloading);
    supervise(&s, client0, first).await?;
    finish(&s).await
}

enum Exit {
    NoWork,
    Stopped,
    /// Server refused an extra connection; run with fewer.
    Limited,
    GaveUp(EngineError),
}

async fn supervise(
    s: &Arc<Shared>,
    client0: Client,
    first: Option<(Arc<Segment>, Option<Response>)>,
) -> Result<(), EngineError> {
    let mut tasks: JoinSet<Exit> = JoinSet::new();
    let spawn_worker = |tasks: &mut JoinSet<Exit>, client: Option<Client>, job| {
        s.active.fetch_add(1, SeqCst);
        let s2 = s.clone();
        tasks.spawn(async move {
            let exit = worker(&s2, client, job).await;
            s2.active.fetch_sub(1, SeqCst);
            exit
        });
    };
    match first {
        Some(job) => spawn_worker(&mut tasks, Some(client0), Some(job)),
        None => {
            if let Some(seg) = claim(s) {
                spawn_worker(&mut tasks, Some(client0), Some((seg, None)));
            }
        }
    }

    let mut tick = tokio::time::interval(Duration::from_millis(150));
    let mut last_err: Option<EngineError> = None;
    loop {
        tokio::select! {
            joined = tasks.join_next(), if !tasks.is_empty() => {
                match joined {
                    Some(Ok(Exit::GaveUp(e))) => {
                        last_err = Some(e);
                        s.target.store(tasks.len().max(1), SeqCst);
                        if tasks.is_empty() {
                            s.set_fatal(last_err.clone().unwrap_or(EngineError::NoProgress));
                        }
                    }
                    Some(Ok(Exit::Limited)) => {
                        s.target.store(tasks.len().max(1), SeqCst);
                    }
                    Some(Err(e)) => s.set_fatal(EngineError::Io(format!("worker crashed: {e}"))),
                    _ => {}
                }
            }
            _ = tick.tick() => {}
        }

        if s.token.is_cancelled() {
            if tasks.is_empty() {
                break;
            }
            continue;
        }
        if s.total_remaining() == 0 && tasks.is_empty() {
            break;
        }
        let target = s.target.load(SeqCst);
        while tasks.len() < target {
            match claim(s) {
                Some(seg) => spawn_worker(&mut tasks, None, Some((seg, None))),
                None => break,
            }
        }
        if tasks.is_empty() && s.total_remaining() > 0 {
            // Nothing can make progress (e.g. open-ended stream was cut).
            s.set_fatal(last_err.clone().unwrap_or(EngineError::NoProgress));
        }
    }

    if let Some(e) = s.fatal.lock().clone() {
        return Err(e);
    }
    if s.token.is_cancelled() {
        return Err(if s.stop.load(SeqCst) == STOP_CANCEL { EngineError::Cancelled } else { EngineError::Paused });
    }
    Ok(())
}

/// Take a waiting range, or split the largest active one in half.
fn claim(s: &Shared) -> Option<Arc<Segment>> {
    let mut segs = s.segments.lock();
    for seg in segs.iter() {
        if !seg.active.load(SeqCst) && seg.remaining() > 0 {
            seg.active.store(true, SeqCst);
            return Some(seg.clone());
        }
    }
    if !s.resumable.load(SeqCst) {
        return None;
    }
    // A victim's in-flight buffer is at most `2 * buffer_size` past its `pos`,
    // so the cut must sit beyond that.
    let guard = s.cfg.min_split.max(2 * s.cfg.buffer_size as u64);
    let victim = segs.iter().filter(|x| x.active.load(SeqCst)).max_by_key(|x| x.remaining())?.clone();
    let mut st = victim.st.lock();
    let rem = st.end.saturating_sub(st.pos);
    if st.end == OPEN_END || rem < 2 * guard {
        return None;
    }
    let mut mid = st.pos + rem / 2;
    let aligned = (mid + 0xFFFF) & !0xFFFF;
    if aligned >= st.pos + guard && aligned + guard <= st.end {
        mid = aligned;
    }
    let new = Segment::new(mid, mid, st.end, true);
    st.end = mid;
    drop(st);
    segs.push(new.clone());
    Some(new)
}

async fn worker(s: &Arc<Shared>, client: Option<Client>, mut job: Option<(Arc<Segment>, Option<Response>)>) -> Exit {
    let client = match client {
        Some(c) => c,
        None => match make_client(&s.cfg) {
            Ok(c) => c,
            Err(e) => return Exit::GaveUp(e),
        },
    };
    let mut failures = 0u32;
    let mut ever_received = false;
    loop {
        let (seg, resp) = match job.take() {
            Some(j) => j,
            None => {
                if s.token.is_cancelled() {
                    return Exit::Stopped;
                }
                match claim(s) {
                    Some(seg) => (seg, None),
                    None => return Exit::NoWork,
                }
            }
        };
        if s.token.is_cancelled() {
            seg.active.store(false, SeqCst);
            return Exit::Stopped;
        }
        let (written, res) = transfer(s, &client, &seg, resp).await;
        seg.active.store(false, SeqCst);
        if written > 0 {
            failures = 0;
            ever_received = true;
        }
        let err = match res {
            Ok(()) => continue,
            Err(e) => e,
        };
        if s.token.is_cancelled() {
            return Exit::Stopped;
        }
        let others = s.active.load(SeqCst) > 1;
        let refused = matches!(err, EngineError::Http(403 | 429 | 503)) || matches!(err, EngineError::Network(_));
        if others && !ever_received && refused {
            return Exit::Limited;
        }
        if !err.is_transient() {
            s.set_fatal(err);
            return Exit::Stopped;
        }
        failures += 1;
        if failures > s.cfg.max_retries {
            return Exit::GaveUp(err);
        }
        let backoff = (0.5 * 2f64.powi(failures as i32 - 1)).min(30.0) * (0.75 + fastrand::f64() * 0.5);
        tokio::select! {
            _ = s.token.cancelled() => return Exit::Stopped,
            _ = tokio::time::sleep(Duration::from_secs_f64(backoff)) => {}
        }
    }
}

/// Stream one range into the file. Returns bytes written plus the outcome.
async fn transfer(s: &Shared, client: &Client, seg: &Segment, resp: Option<Response>) -> (u64, Result<(), EngineError>) {
    let resp = match resp {
        Some(r) => r,
        None => match range_request(s, client, seg).await {
            Ok(Some(r)) => r,
            Ok(None) => return (0, Ok(())),
            Err(e) => return (0, Err(e)),
        },
    };
    let mut stream = resp.bytes_stream();
    let cap = s.cfg.buffer_size;
    let mut buf = BytesMut::with_capacity(cap + 64 * 1024);
    let mut written = 0u64;
    loop {
        let next = tokio::select! {
            _ = s.token.cancelled() => return (written, Err(EngineError::Cancelled)),
            n = tokio::time::timeout(s.cfg.read_timeout, stream.next()) => n,
        };
        match next {
            Err(_) => {
                let r = flush(s, seg, &mut buf).await;
                written += r.as_ref().map(|x| x.0).unwrap_or(0);
                return (written, r.and(Err(EngineError::NoProgress)));
            }
            Ok(Some(Err(e))) => {
                let r = flush(s, seg, &mut buf).await;
                written += r.as_ref().map(|x| x.0).unwrap_or(0);
                return (written, r.and(Err(EngineError::Network(e.to_string()))));
            }
            Ok(None) => {
                let r = flush(s, seg, &mut buf).await;
                written += r.as_ref().map(|x| x.0).unwrap_or(0);
                if let Err(e) = r {
                    return (written, Err(e));
                }
                let mut st = seg.st.lock();
                if st.end == OPEN_END {
                    // Size was unknown: the end of the body is the end of the file.
                    st.end = st.pos;
                    let size = st.pos;
                    drop(st);
                    s.meta.lock().size = Some(size);
                    return (written, Ok(()));
                }
                if st.pos < st.end {
                    return (written, Err(EngineError::Network("connection closed early".into())));
                }
                return (written, Ok(()));
            }
            Ok(Some(Ok(chunk))) => {
                for l in &s.limiters {
                    l.acquire(chunk.len()).await;
                }
                buf.extend_from_slice(&chunk);
                let reached_end = {
                    let st = seg.st.lock();
                    st.pos + buf.len() as u64 >= st.end
                };
                if buf.len() >= cap || reached_end {
                    match flush(s, seg, &mut buf).await {
                        Ok((n, done)) => {
                            written += n;
                            if done {
                                // Our tail was stolen or we finished: drop the connection.
                                return (written, Ok(()));
                            }
                        }
                        Err(e) => return (written, Err(e)),
                    }
                }
            }
        }
    }
}

/// Write buffered bytes (clamped to the segment's current end). Returns (bytes, segment done).
async fn flush(s: &Shared, seg: &Segment, buf: &mut BytesMut) -> Result<(u64, bool), EngineError> {
    let (pos, n) = {
        let st = seg.st.lock();
        (st.pos, (buf.len() as u64).min(st.end.saturating_sub(st.pos)) as usize)
    };
    if n > 0 {
        let data = buf.split_to(n).freeze();
        let file = s.file.lock().clone().ok_or_else(|| EngineError::Io("file closed".into()))?;
        tokio::task::spawn_blocking(move || storage::write_all_at(&file, pos, &data))
            .await
            .map_err(|e| EngineError::Io(e.to_string()))?
            .map_err(io_err)?;
        seg.st.lock().pos += n as u64;
        s.downloaded.fetch_add(n as u64, Relaxed);
    }
    buf.clear();
    let st = seg.st.lock();
    Ok((n as u64, st.pos >= st.end))
}

async fn range_request(s: &Shared, client: &Client, seg: &Segment) -> Result<Option<Response>, EngineError> {
    let (pos, end) = {
        let st = seg.st.lock();
        (st.pos, st.end)
    };
    if pos >= end {
        return Ok(None);
    }
    let range = if end == OPEN_END { format!("bytes={pos}-") } else { format!("bytes={pos}-{}", end - 1) };
    let (if_range, size) = {
        let m = s.meta.lock();
        let strong_etag = m.etag.clone().filter(|e| !e.starts_with("W/"));
        (strong_etag.or(m.last_modified.clone()), m.size)
    };
    let mut rb = client.get(s.url()).headers(extra_headers(&s.req)).header(header::RANGE, range);
    if let Some(v) = &if_range {
        rb = rb.header(header::IF_RANGE, v);
    }
    let resp = send(s, rb).await?;
    match resp.status() {
        StatusCode::PARTIAL_CONTENT => {
            let (total, _) = parse_size(&resp, pos);
            if total.is_some() && size.is_some() && total != size {
                return Err(EngineError::ValidatorMismatch);
            }
            match content_range_start(resp.headers()) {
                Some(start) if start == pos => Ok(Some(resp)),
                _ => Err(EngineError::Network("server sent the wrong range".into())),
            }
        }
        StatusCode::OK if if_range.is_some() => Err(EngineError::ValidatorMismatch),
        StatusCode::OK => Err(EngineError::Network("server ignored the range request".into())),
        code => Err(EngineError::Http(code.as_u16())),
    }
}

async fn first_request(s: &Shared, client: &Client) -> Result<Response, EngineError> {
    let mut attempt = 0u32;
    loop {
        let rb = client.get(&s.req.url).headers(extra_headers(&s.req)).header(header::RANGE, "bytes=0-");
        let res = send(s, rb).await.and_then(|r| match r.status() {
            StatusCode::OK | StatusCode::PARTIAL_CONTENT => Ok(r),
            code => Err(EngineError::Http(code.as_u16())),
        });
        match res {
            Ok(r) => return Ok(r),
            Err(e) if e.is_transient() && attempt < s.cfg.max_retries => {
                attempt += 1;
                let wait = (0.5 * 2f64.powi(attempt as i32 - 1)).min(30.0);
                tokio::select! {
                    _ = s.token.cancelled() => return Err(EngineError::Cancelled),
                    _ = tokio::time::sleep(Duration::from_secs_f64(wait)) => {}
                }
            }
            Err(e) => return Err(e),
        }
    }
}

async fn send(s: &Shared, rb: reqwest::RequestBuilder) -> Result<Response, EngineError> {
    let timeout = s.cfg.connect_timeout + s.cfg.read_timeout;
    tokio::select! {
        _ = s.token.cancelled() => Err(EngineError::Cancelled),
        r = tokio::time::timeout(timeout, rb.send()) => match r {
            Err(_) => Err(EngineError::NoProgress),
            Ok(Err(e)) => Err(EngineError::Network(e.to_string())),
            Ok(Ok(resp)) => Ok(resp),
        },
    }
}

async fn finish(s: &Shared) -> Result<Completed, EngineError> {
    s.set_phase(Phase::Finishing);
    let file = s.file.lock().take();
    if let Some(f) = file {
        tokio::task::spawn_blocking(move || f.sync_all())
            .await
            .map_err(|e| EngineError::Io(e.to_string()))?
            .map_err(io_err)?;
    }
    let (final_path, size) = {
        let m = s.meta.lock();
        (m.file_path.clone().ok_or_else(|| EngineError::Io("no file".into()))?, m.size)
    };
    let part = part_path(&final_path);
    let size = size.unwrap_or_else(|| s.downloaded.load(SeqCst));
    // Truncate if the size was unknown and we pre-allocated nothing (no-op otherwise).
    let target = if final_path.exists() {
        let dir = final_path.parent().map(PathBuf::from).unwrap_or_default();
        let name = final_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        unique_path(&dir, &name)
    } else {
        final_path
    };
    let t2 = target.clone();
    tokio::task::spawn_blocking(move || std::fs::rename(&part, &t2))
        .await
        .map_err(|e| EngineError::Io(e.to_string()))?
        .map_err(io_err)?;
    s.meta.lock().file_path = Some(target.clone());
    Ok(Completed { path: target, size })
}

pub(crate) async fn probe(cfg: &EngineConfig, req: &DownloadRequest) -> Result<ProbeInfo, EngineError> {
    url::Url::parse(&req.url).map_err(|e| EngineError::BadUrl(e.to_string()))?;
    let client = make_client(cfg)?;
    let fut = client.get(&req.url).headers(extra_headers(req)).header(header::RANGE, "bytes=0-0").send();
    let resp = tokio::time::timeout(cfg.connect_timeout + cfg.read_timeout, fut)
        .await
        .map_err(|_| EngineError::NoProgress)?
        .map_err(|e| EngineError::Network(e.to_string()))?;
    let status = resp.status();
    if !(status == StatusCode::OK || status == StatusCode::PARTIAL_CONTENT) {
        return Err(EngineError::Http(status.as_u16()));
    }
    let (size, resumable) = parse_size(&resp, 0);
    Ok(ProbeInfo {
        final_url: resp.url().to_string(),
        file_name: detect_name(req.file_name.as_deref(), &resp),
        size,
        resumable,
        content_type: header_str(resp.headers(), header::CONTENT_TYPE),
    })
}

fn make_client(cfg: &EngineConfig) -> Result<Client, EngineError> {
    Client::builder()
        .user_agent(cfg.user_agent.clone())
        .connect_timeout(cfg.connect_timeout)
        .http1_only()
        .tcp_nodelay(true)
        .pool_max_idle_per_host(1)
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| EngineError::Network(e.to_string()))
}

fn extra_headers(req: &DownloadRequest) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (k, v) in &req.headers {
        if let (Ok(name), Ok(val)) = (HeaderName::from_bytes(k.as_bytes()), HeaderValue::from_str(v)) {
            map.insert(name, val);
        }
    }
    map
}

fn header_str(h: &HeaderMap, name: HeaderName) -> Option<String> {
    h.get(name).and_then(|v| v.to_str().ok()).map(str::to_owned)
}

/// (total size, range support) from a response to a request starting at `start`.
fn parse_size(resp: &Response, start: u64) -> (Option<u64>, bool) {
    if resp.status() == StatusCode::PARTIAL_CONTENT {
        let total = header_str(resp.headers(), header::CONTENT_RANGE)
            .and_then(|v| v.rsplit('/').next().and_then(|t| t.trim().parse::<u64>().ok()));
        (total, true)
    } else {
        (resp.content_length().map(|l| l + start), false)
    }
}

fn content_range_start(h: &HeaderMap) -> Option<u64> {
    let v = header_str(h, header::CONTENT_RANGE)?;
    let v = v.trim().strip_prefix("bytes")?.trim();
    v.split('-').next()?.trim().parse().ok()
}

fn detect_name(explicit: Option<&str>, resp: &Response) -> String {
    let name = explicit
        .map(str::to_owned)
        .or_else(|| header_str(resp.headers(), header::CONTENT_DISPOSITION).and_then(|v| filename::from_content_disposition(&v)))
        .or_else(|| filename::file_name_from_url(resp.url().as_str()))
        .unwrap_or_else(|| "download".into());
    filename::sanitize_file_name(&name)
}

fn io_err(e: std::io::Error) -> EngineError {
    EngineError::Io(e.to_string())
}
