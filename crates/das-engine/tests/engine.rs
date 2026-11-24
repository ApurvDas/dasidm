use axum::body::Body;
use axum::extract::Query;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use axum::routing::get;
use bytes::Bytes;
use das_engine::{DownloadRequest, Engine, EngineConfig, EngineError};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

static IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);
static MAX_IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);
static REQUESTS: AtomicUsize = AtomicUsize::new(0);

/// Per-test server state, keyed by the `key` query parameter, so tests running at the same time
/// don't see each other's requests.
#[derive(Default)]
struct Lab {
    in_flight: usize,
    max_in_flight: usize,
    max_in_flight_late: usize,
    first_at: Option<Instant>,
    misbehaved: bool,
    refused: usize,
}
static LABS: std::sync::LazyLock<parking_lot::Mutex<HashMap<String, Lab>>> = std::sync::LazyLock::new(Default::default);

struct LabGuard(String);
impl Drop for LabGuard {
    fn drop(&mut self) {
        if let Some(l) = LABS.lock().get_mut(&self.0) {
            l.in_flight -= 1;
        }
    }
}

fn byte_at(i: u64) -> u8 {
    ((i.wrapping_mul(31) + i / 7) % 251) as u8
}

fn expected(size: u64) -> Vec<u8> {
    (0..size).map(byte_at).collect()
}

struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        IN_FLIGHT.fetch_sub(1, Ordering::SeqCst);
    }
}

fn parse_range(h: &HeaderMap) -> Option<(u64, Option<u64>)> {
    let v = h.get(header::RANGE)?.to_str().ok()?.strip_prefix("bytes=")?;
    let (a, b) = v.split_once('-')?;
    Some((a.parse().ok()?, b.parse().ok()))
}

async fn serve_file(q: HashMap<String, String>, h: HeaderMap, ranges: bool) -> Response {
    let size: u64 = q["size"].parse().unwrap();
    // Lab behaviours (only with `key`):
    //   refuse=N:MS  more than N at once get 503 + Retry-After: 1 during the first MS ms
    //   stall=1      the first request that starts past byte 0 sends 32 KiB, then nothing
    //   trickle=US   the first request that starts past byte 0 sends 16 KiB per US microseconds
    let mut misbehave = false;
    let mut lab_guard = None;
    if let Some(key) = q.get("key") {
        let start = if ranges { parse_range(&h).map(|r| r.0).unwrap_or(0) } else { 0 };
        let mut labs = LABS.lock();
        let lab = labs.entry(key.clone()).or_default();
        let first_at = *lab.first_at.get_or_insert_with(Instant::now);
        if let Some((n, ms)) = q.get("refuse").and_then(|v| v.split_once(':')) {
            let (n, ms): (usize, u64) = (n.parse().unwrap(), ms.parse().unwrap());
            if first_at.elapsed() < Duration::from_millis(ms) && lab.in_flight >= n {
                lab.refused += 1;
                return Response::builder()
                    .status(StatusCode::SERVICE_UNAVAILABLE)
                    .header(header::RETRY_AFTER, "1")
                    .body(Body::empty())
                    .unwrap();
            }
        }
        lab.in_flight += 1;
        lab.max_in_flight = lab.max_in_flight.max(lab.in_flight);
        if first_at.elapsed() > Duration::from_millis(1500) {
            lab.max_in_flight_late = lab.max_in_flight_late.max(lab.in_flight);
        }
        if start > 0 && !lab.misbehaved && (q.contains_key("stall") || q.contains_key("trickle")) {
            lab.misbehaved = true;
            misbehave = true;
        }
        lab_guard = Some(LabGuard(key.clone()));
    }
    let stall = misbehave && q.contains_key("stall");
    let trickle = if misbehave { q.get("trickle").and_then(|d| d.parse::<u64>().ok()).map(Duration::from_micros) } else { None };
    let mut delay = Duration::from_micros(q.get("delay").and_then(|d| d.parse().ok()).unwrap_or(0));
    if q.contains_key("vary") {
        // Uneven lines: every connection is 1x to 4x slower than the base, in turn.
        delay *= 1 + (REQUESTS.fetch_add(1, Ordering::SeqCst) % 4) as u32;
    }
    let range = if ranges { parse_range(&h) } else { None };
    let (start, end, status) = match range {
        Some((a, b)) => (a, b.unwrap_or(size - 1).min(size - 1), StatusCode::PARTIAL_CONTENT),
        None => (0, size - 1, StatusCode::OK),
    };
    let n = IN_FLIGHT.fetch_add(1, Ordering::SeqCst) + 1;
    MAX_IN_FLIGHT.fetch_max(n, Ordering::SeqCst);
    let guard = Guard;
    let stream = futures_util::stream::unfold((start, (guard, lab_guard)), move |(pos, g)| async move {
        if pos > end {
            return None;
        }
        let n = (end + 1 - pos).min(16 * 1024);
        if stall && pos >= start + 32 * 1024 {
            // Headers and a little data, then silence: the connection stays open.
            std::future::pending::<()>().await;
        }
        if let Some(t) = trickle {
            tokio::time::sleep(t).await;
        } else if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
        let chunk: Vec<u8> = (pos..pos + n).map(byte_at).collect();
        Some((Ok::<Bytes, std::io::Error>(Bytes::from(chunk)), (pos + n, g)))
    });
    let mut b = Response::builder()
        .status(status)
        .header(header::CONTENT_LENGTH, end - start + 1)
        .header(header::ETAG, "\"v1\"")
        .header(header::CONTENT_DISPOSITION, "attachment; filename=\"test file.bin\"");
    if status == StatusCode::PARTIAL_CONTENT {
        b = b.header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{size}"));
    }
    b.body(Body::from_stream(stream)).unwrap()
}

async fn server() -> String {
    let app = axum::Router::new()
        .route("/file", get(|Query(q): Query<HashMap<String, String>>, h: HeaderMap| serve_file(q, h, true)))
        .route("/norange", get(|Query(q): Query<HashMap<String, String>>, h: HeaderMap| serve_file(q, h, false)))
        .route("/missing", get(|| async { StatusCode::NOT_FOUND }))
        .route(
            "/page",
            get(|| async {
                Response::builder()
                    .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
                    .body(Body::from("<!doctype html><title>Share</title>"))
                    .unwrap()
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}")
}

fn engine() -> Engine {
    Engine::new(EngineConfig { min_split: 256 * 1024, buffer_size: 64 * 1024, max_retries: 2, ..Default::default() })
}

fn req(url: String, dir: &std::path::Path, conns: usize) -> DownloadRequest {
    DownloadRequest { url, headers: vec![], save_dir: dir.to_path_buf(), file_name: None, connections: conns, allow_html: false }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn web_pages_are_refused_unless_asked() {
    let base = server().await;
    let dir = tempfile::tempdir().unwrap();
    let h = engine().start(req(format!("{base}/page"), dir.path(), 4), None, vec![]);
    assert_eq!(h.wait().await.unwrap_err(), EngineError::WebPage);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0, "no file may be created for a refused page");

    let mut r = req(format!("{base}/page"), dir.path(), 4);
    r.allow_html = true;
    let done = engine().start(r, None, vec![]).wait().await.unwrap();
    assert!(std::fs::read_to_string(done.path).unwrap().contains("<title>Share</title>"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn segmented_download_matches() {
    let base = server().await;
    let dir = tempfile::tempdir().unwrap();
    let size = 8 * 1024 * 1024;
    let h = engine().start(req(format!("{base}/file?size={size}&delay=300"), dir.path(), 8), None, vec![]);
    let done = h.wait().await.unwrap();
    assert_eq!(done.size, size);
    assert_eq!(done.path.file_name().unwrap(), "test file.bin");
    assert!(std::fs::read(&done.path).unwrap() == expected(size), "content mismatch");
    assert!(MAX_IN_FLIGHT.load(Ordering::SeqCst) >= 4, "expected parallel connections");
    assert!(!dir.path().join("test file.bin.dasdl").exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn fast_lines_steal_from_slow_ones() {
    let base = server().await;
    let dir = tempfile::tempdir().unwrap();
    // Delays well above the ~15 ms Windows timer resolution, so 1x..4x lines really differ.
    let size = 4 * 1024 * 1024;
    let eng = Engine::new(EngineConfig { min_split: 64 * 1024, buffer_size: 16 * 1024, max_retries: 2, ..Default::default() });
    let h = eng.start(req(format!("{base}/file?size={size}&delay=16000&vary=1"), dir.path(), 8), None, vec![]);
    let done = h.wait().await.unwrap();
    assert!(std::fs::read(&done.path).unwrap() == expected(size), "content mismatch");
    // Lines that finished early split the slow lines' parts, so there are more parts than lines.
    let snap = h.snapshot();
    let parts = snap.segments.len();
    let sizes: Vec<u64> = snap.segments.iter().map(|s| (s.end - s.start) / 1024).collect();
    assert!(parts > 8, "expected work stealing to split parts, got {parts}: sizes KiB {sizes:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn no_range_server_uses_one_connection() {
    let base = server().await;
    let dir = tempfile::tempdir().unwrap();
    let size = 1024 * 1024 + 17;
    let h = engine().start(req(format!("{base}/norange?size={size}"), dir.path(), 8), None, vec![]);
    let done = h.wait().await.unwrap();
    assert_eq!(std::fs::read(&done.path).unwrap(), expected(size));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pause_and_resume() {
    let base = server().await;
    let dir = tempfile::tempdir().unwrap();
    let size = 16 * 1024 * 1024;
    let eng = engine();
    let r = req(format!("{base}/file?size={size}&delay=800"), dir.path(), 4);
    let h = eng.start(r.clone(), None, vec![]);
    while h.downloaded() < 2 * 1024 * 1024 {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    h.pause();
    assert_eq!(h.wait().await.unwrap_err(), EngineError::Paused);
    let state = h.resume_state().unwrap();
    assert!(state.resumable && !state.remaining.is_empty());
    assert!(state.downloaded() >= 2 * 1024 * 1024 && state.downloaded() < size);

    let h2 = eng.start(r, Some(state), vec![]);
    let done = h2.wait().await.unwrap();
    assert_eq!(std::fs::read(&done.path).unwrap(), expected(size));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_file_fails_fast() {
    let base = server().await;
    let dir = tempfile::tempdir().unwrap();
    let h = engine().start(req(format!("{base}/missing"), dir.path(), 4), None, vec![]);
    assert_eq!(h.wait().await.unwrap_err(), EngineError::Http(404));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn speed_limit_is_respected() {
    let base = server().await;
    let dir = tempfile::tempdir().unwrap();
    let eng = engine();
    eng.global_limiter().set_rate(1024 * 1024);
    let started = Instant::now();
    let h = eng.start(req(format!("{base}/file?size={}", 1536 * 1024), dir.path(), 4), None, vec![]);
    h.wait().await.unwrap();
    // 1.5 MiB at 1 MiB/s with a 256 KiB burst: at least ~1.2 s.
    assert!(started.elapsed() >= Duration::from_millis(1100), "took {:?}", started.elapsed());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn probe_reports_size_and_name() {
    let base = server().await;
    let dir = tempfile::tempdir().unwrap();
    let info = engine().probe(&req(format!("{base}/file?size=5000"), dir.path(), 1)).await.unwrap();
    assert_eq!(info.size, Some(5000));
    assert!(info.resumable);
    assert_eq!(info.file_name, "test file.bin");
}

fn lab(key: &str) -> (usize, usize, usize) {
    let labs = LABS.lock();
    let l = &labs[key];
    (l.max_in_flight, l.max_in_flight_late, l.refused)
}

/// Settings that make the slow-line rules act within a test's time.
fn quick_engine() -> Engine {
    Engine::new(EngineConfig {
        min_split: 256 * 1024,
        buffer_size: 64 * 1024,
        max_retries: 4,
        stall_timeout: Duration::from_secs(1),
        slow_grace: Duration::from_secs(1),
        regrow_after: Duration::from_millis(800),
        ..Default::default()
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_stalled_line_is_replaced() {
    let base = server().await;
    let dir = tempfile::tempdir().unwrap();
    let size = 8 * 1024 * 1024;
    let started = Instant::now();
    let h = quick_engine().start(req(format!("{base}/file?size={size}&delay=300&key=stall&stall=1"), dir.path(), 4), None, vec![]);
    let done = tokio::time::timeout(Duration::from_secs(20), h.wait()).await.expect("download hung on the stalled line").unwrap();
    assert!(std::fs::read(&done.path).unwrap() == expected(size), "content mismatch");
    // Without replacement the stalled range waits out the 30 s read timeout.
    assert!(started.elapsed() < Duration::from_secs(8), "took {:?}", started.elapsed());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_trickling_line_is_replaced() {
    let base = server().await;
    let dir = tempfile::tempdir().unwrap();
    let size = 8 * 1024 * 1024;
    let started = Instant::now();
    // One line gets 16 KiB every 150 ms (about 100 KB/s); the others run at full speed.
    let h = quick_engine().start(req(format!("{base}/file?size={size}&delay=300&key=trickle&trickle=150000"), dir.path(), 4), None, vec![]);
    let done = tokio::time::timeout(Duration::from_secs(30), h.wait()).await.expect("too slow").unwrap();
    assert!(std::fs::read(&done.path).unwrap() == expected(size), "content mismatch");
    // Its last unsplittable 512 KiB alone would take about 5 s at that pace.
    assert!(started.elapsed() < Duration::from_secs(4), "took {:?}", started.elapsed());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn lines_come_back_after_the_server_stops_refusing() {
    let base = server().await;
    let dir = tempfile::tempdir().unwrap();
    let size = 24 * 1024 * 1024;
    // At most 2 at once for the first second (503 with Retry-After: 1), then no limit.
    let h = quick_engine().start(req(format!("{base}/file?size={size}&delay=4000&key=refuse&refuse=2:1000"), dir.path(), 6), None, vec![]);
    let done = tokio::time::timeout(Duration::from_secs(60), h.wait()).await.expect("too slow").unwrap();
    assert!(std::fs::read(&done.path).unwrap() == expected(size), "content mismatch");
    let (_, late, refused) = lab("refuse");
    assert!(refused > 0, "the server should have refused some lines");
    assert!(late >= 5, "lines should grow back towards 6 once refusals stop; peak later was {late}");
}
