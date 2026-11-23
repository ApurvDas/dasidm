//! Firefox → DasIDM.
//!
//! The DasIDM extension talks to Firefox's "native messaging" host `dev.dasidm.host`, which is
//! this very program: Firefox starts `DasIDM.exe <manifest> dasidm@dasidm.local`, `main()` sees
//! that before any window or WebView exists and runs [`run_host`]. The helper reads Firefox's
//! frames (a 4-byte length and JSON) from stdin and relays each one, as a line of JSON, to the
//! running DasIDM over a named pipe that only this Windows user's local processes can reach. If
//! DasIDM isn't running, the helper starts it in the tray first.
//!
//! [`register`] (run on every start) writes the host manifest, points Firefox at it in
//! HKCU\Software\Mozilla\NativeMessagingHosts and unpacks the extension next to it.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const HOST_NAME: &str = "dev.dasidm.host";
pub const FIREFOX_ID: &str = "dasidm@dasidm.local";
/// Biggest message either way (Firefox allows 4 GB in and 1 MB out; links are small).
const MAX_FRAME: usize = 1 << 20;

/// The pipe for this Windows user.
pub fn pipe_name() -> String {
    let user: String = std::env::var("USERNAME")
        .unwrap_or_default()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    format!(r"\\.\pipe\dasidm-browser-{user}")
}

/// Firefox starts the host with the manifest path and the extension's id as arguments.
pub fn is_host_launch(args: &[String]) -> bool {
    args.iter().skip(1).any(|a| a == FIREFOX_ID) || args.get(1).is_some_and(|a| a.ends_with(&format!("{HOST_NAME}.json")))
}

// ---- native messaging frames ----

pub fn read_frame(r: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut len = [0u8; 4];
    match r.read_exact(&mut len) {
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
        Ok(()) => {}
    }
    let n = u32::from_le_bytes(len) as usize;
    if n > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "message too big"));
    }
    let mut buf = vec![0; n];
    r.read_exact(&mut buf)?;
    Ok(Some(buf))
}

pub fn write_frame(w: &mut impl Write, data: &[u8]) -> io::Result<()> {
    w.write_all(&(data.len() as u32).to_le_bytes())?;
    w.write_all(data)?;
    w.flush()
}

// ---- the helper (Firefox's side of the pipe) ----

/// Helper mode: relay Firefox's messages until it closes the connection.
pub fn run_host() -> i32 {
    allow_focus();
    let mut input = io::stdin().lock();
    let mut out = io::stdout().lock();
    loop {
        let msg = match read_frame(&mut input) {
            Ok(Some(m)) => m,
            Ok(None) => return 0,
            Err(_) => return 1,
        };
        let reply = match serde_json::from_slice::<Value>(&msg) {
            Ok(v) => relay(&pipe_name(), &v, true).unwrap_or_else(|e| json!({ "ok": false, "error": e })),
            Err(_) => json!({ "ok": false, "error": "not JSON" }),
        };
        if write_frame(&mut out, reply.to_string().as_bytes()).is_err() {
            return 1;
        }
    }
}

/// Firefox is in front; its child may hand the foreground on, so DasIDM's window can come up
/// over it instead of only flashing in the taskbar.
fn allow_focus() {
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(windows_sys::Win32::UI::WindowsAndMessaging::ASFW_ANY);
    }
}

/// Send one message to DasIDM and wait for its answer, starting DasIDM if needed.
pub fn relay(pipe: &str, msg: &Value, may_start: bool) -> Result<Value, String> {
    let line = format!("{msg}\n");
    let deadline = Instant::now() + Duration::from_secs(8);
    let mut started = false;
    loop {
        match std::fs::OpenOptions::new().read(true).write(true).open(pipe) {
            Ok(mut p) => {
                p.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
                let mut answer = String::new();
                BufReader::new(p).take(MAX_FRAME as u64).read_line(&mut answer).map_err(|e| e.to_string())?;
                return serde_json::from_str(answer.trim_end()).map_err(|_| "DasIDM sent a bad answer".to_string());
            }
            Err(e) => {
                // Not there: start it in the tray (once). Busy (231): another message is going.
                if may_start && !started && e.kind() == io::ErrorKind::NotFound {
                    started = true;
                    if let Ok(exe) = std::env::current_exe() {
                        let _ = std::process::Command::new(exe).arg("--background").spawn();
                    }
                }
                if Instant::now() > deadline {
                    return Err(if started { "DasIDM didn't start in time".into() } else { format!("DasIDM isn't reachable ({e})") });
                }
                std::thread::sleep(Duration::from_millis(120));
            }
        }
    }
}

// ---- DasIDM's side of the pipe ----

/// A download Firefox hands over.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserDownload {
    pub url: String,
    #[serde(default)]
    pub file_name: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub referrer: Option<String>,
    #[serde(default)]
    pub cookie: Option<String>,
    #[serde(default)]
    pub user_agent: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Request {
    Ping,
    Download(BrowserDownload),
}

/// What the Add sheet opens with: a link, and for browser downloads the headers that let the
/// server hand the same file to DasIDM (the login cookie, the page it came from).
#[derive(Clone, Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PendingAdd {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub file_name: Option<String>,
    pub size: Option<u64>,
    /// "Firefox" for a browser hand-off.
    pub from: Option<String>,
}

impl PendingAdd {
    pub fn link(url: impl Into<String>) -> Self {
        Self { url: url.into(), ..Default::default() }
    }
}

impl BrowserDownload {
    /// Checked and turned into what the Add sheet needs. Only the headers that identify the
    /// user's own session with that site are kept.
    pub fn into_pending(self) -> Result<PendingAdd, String> {
        let url = self.url.trim().to_string();
        if !(url.starts_with("http://") || url.starts_with("https://")) || url.len() > 8192 || url.contains(char::is_whitespace) {
            return Err("not a download link".into());
        }
        let clean = |v: Option<String>| v.map(|s| s.replace(['\r', '\n'], "")).filter(|s| !s.is_empty() && s.len() < 16 * 1024);
        let mut headers = Vec::new();
        if let Some(c) = clean(self.cookie) {
            headers.push(("Cookie".into(), c));
        }
        if let Some(r) = clean(self.referrer).filter(|r| r.starts_with("http")) {
            headers.push(("Referer".into(), r));
        }
        if let Some(ua) = clean(self.user_agent) {
            headers.push(("User-Agent".into(), ua));
        }
        let file_name = self.file_name.map(|n| n.replace(['\\', '/', ':', '*', '?', '"', '<', '>', '|'], "_")).filter(|n| !n.trim().is_empty());
        Ok(PendingAdd { url, headers, file_name, size: self.size.filter(|s| *s > 0), from: Some("Firefox".into()) })
    }
}

pub type Handler = Arc<dyn Fn(Request) -> Value + Send + Sync>;

/// Answer the helper's messages until DasIDM exits.
pub async fn serve(pipe: String, handler: Handler) -> io::Result<()> {
    use tokio::net::windows::named_pipe::ServerOptions;
    // first_pipe_instance: never answer on a pipe someone else made first.
    let mut server = ServerOptions::new().first_pipe_instance(true).reject_remote_clients(true).create(&pipe)?;
    loop {
        server.connect().await?;
        let client = server;
        server = ServerOptions::new().reject_remote_clients(true).create(&pipe)?;
        let handler = handler.clone();
        tokio::spawn(async move {
            let _ = answer(client, handler).await;
        });
    }
}

async fn answer(pipe: tokio::net::windows::named_pipe::NamedPipeServer, handler: Handler) -> io::Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
    let (r, mut w) = tokio::io::split(pipe);
    let mut line = String::new();
    tokio::io::BufReader::new(r.take(MAX_FRAME as u64)).read_line(&mut line).await?;
    let reply = match serde_json::from_str::<Request>(line.trim_end()) {
        Ok(req) => handler(req),
        Err(_) => json!({ "ok": false, "error": "unknown message" }),
    };
    w.write_all(format!("{reply}\n").as_bytes()).await?;
    w.flush().await
}

// ---- registration with Firefox ----

/// Where the manifest and the unpacked extension live.
pub fn browser_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("DasIDM").join("browser")
}

const REG_KEY: &str = r"Software\Mozilla\NativeMessagingHosts\dev.dasidm.host";

/// The extension's files, built into DasIDM so the folder can be loaded in Firefox.
const EXTENSION: &[(&str, &[u8])] = &[
    ("manifest.json", include_bytes!("../../../extension/firefox/manifest.json")),
    ("background.js", include_bytes!("../../../extension/firefox/background.js")),
    ("rules.js", include_bytes!("../../../extension/firefox/rules.js")),
    ("content.js", include_bytes!("../../../extension/firefox/content.js")),
    ("popup.html", include_bytes!("../../../extension/firefox/popup.html")),
    ("popup.css", include_bytes!("../../../extension/firefox/popup.css")),
    ("popup.js", include_bytes!("../../../extension/firefox/popup.js")),
    ("icons/32.png", include_bytes!("../../../extension/firefox/icons/32.png")),
    ("icons/64.png", include_bytes!("../../../extension/firefox/icons/64.png")),
    ("icons/128.png", include_bytes!("../../../extension/firefox/icons/128.png")),
];

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Registration {
    pub registered: bool,
    pub manifest: String,
    pub extension_dir: String,
    pub error: Option<String>,
}

/// Make Firefox find this DasIDM. Cheap when nothing changed (files compared, key read).
pub fn register() -> Registration {
    let dir = browser_dir();
    let manifest = dir.join(format!("{HOST_NAME}.json"));
    let ext = dir.join("firefox");
    let result = (|| -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let body = serde_json::to_string_pretty(&json!({
            "name": HOST_NAME,
            "description": "DasIDM download manager",
            "path": exe,
            "type": "stdio",
            "allowed_extensions": [FIREFOX_ID],
        }))
        .map_err(|e| e.to_string())?;
        write_if_changed(&manifest, body.as_bytes())?;
        for (name, bytes) in EXTENSION {
            write_if_changed(&ext.join(name), bytes)?;
        }
        registry::set_default_if_changed(REG_KEY, &manifest.to_string_lossy())
    })();
    Registration {
        registered: result.is_ok() && registry::get_default(REG_KEY).is_some_and(|v| Path::new(&v) == manifest),
        manifest: manifest.to_string_lossy().into_owned(),
        extension_dir: ext.to_string_lossy().into_owned(),
        error: result.err(),
    }
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if std::fs::read(path).is_ok_and(|old| old == bytes) {
        return Ok(());
    }
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(windows)]
mod registry {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::*;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn get_default(key: &str) -> Option<String> {
        let mut buf = vec![0u16; 1024];
        let mut len = (buf.len() * 2) as u32;
        let k = wide(key);
        let rc = unsafe { RegGetValueW(HKEY_CURRENT_USER, k.as_ptr(), std::ptr::null(), RRF_RT_REG_SZ, std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut len) };
        if rc != ERROR_SUCCESS {
            return None;
        }
        let n = (len as usize / 2).saturating_sub(1);
        Some(String::from_utf16_lossy(&buf[..n]))
    }

    pub fn set_default_if_changed(key: &str, value: &str) -> Result<(), String> {
        if get_default(key).as_deref() == Some(value) {
            return Ok(());
        }
        let (k, v) = (wide(key), wide(value));
        let mut h: HKEY = std::ptr::null_mut();
        let rc = unsafe { RegCreateKeyExW(HKEY_CURRENT_USER, k.as_ptr(), 0, std::ptr::null(), 0, KEY_SET_VALUE, std::ptr::null(), &mut h, std::ptr::null_mut()) };
        if rc != ERROR_SUCCESS {
            return Err(format!("Windows refused to register the Firefox helper ({rc})"));
        }
        let rc = unsafe { RegSetValueExW(h, std::ptr::null(), 0, REG_SZ, v.as_ptr().cast(), (v.len() * 2) as u32) };
        unsafe { RegCloseKey(h) };
        if rc == ERROR_SUCCESS { Ok(()) } else { Err(format!("Windows refused to register the Firefox helper ({rc})")) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip() {
        let mut buf = Vec::new();
        write_frame(&mut buf, br#"{"type":"ping"}"#).unwrap();
        write_frame(&mut buf, b"{}").unwrap();
        assert_eq!(&buf[..4], &15u32.to_le_bytes());
        let mut r = &buf[..];
        assert_eq!(read_frame(&mut r).unwrap().unwrap(), br#"{"type":"ping"}"#);
        assert_eq!(read_frame(&mut r).unwrap().unwrap(), b"{}");
        assert!(read_frame(&mut r).unwrap().is_none(), "end of input is a clean stop");
        let huge = (MAX_FRAME as u32 + 1).to_le_bytes();
        assert!(read_frame(&mut &huge[..]).is_err());
    }

    #[test]
    fn firefox_launch_is_recognised() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(is_host_launch(&a(&["DasIDM.exe", r"C:\x\dev.dasidm.host.json", FIREFOX_ID])));
        assert!(!is_host_launch(&a(&["DasIDM.exe", "--background"])));
        assert!(!is_host_launch(&a(&["DasIDM.exe", "https://example.org/a.zip"])));
    }

    #[test]
    fn browser_downloads_are_checked() {
        let d = |url: &str| BrowserDownload { url: url.into(), file_name: Some("a/b:c.zip".into()), size: Some(0), referrer: Some("https://site/page".into()), cookie: Some("s=1\r\nX-Evil: 1".into()), user_agent: None };
        let p = d("https://example.org/a.zip").into_pending().unwrap();
        assert_eq!(p.file_name.as_deref(), Some("a_b_c.zip"));
        assert_eq!(p.size, None);
        assert_eq!(p.headers, vec![("Cookie".into(), "s=1X-Evil: 1".into()), ("Referer".into(), "https://site/page".into())]);
        assert_eq!(p.from.as_deref(), Some("Firefox"));
        assert!(d("file:///C:/Windows/win.ini").into_pending().is_err());
        assert!(d("javascript:alert(1)").into_pending().is_err());
        assert!(d("https://a b").into_pending().is_err());
    }

    #[test]
    fn helper_and_app_talk_over_the_pipe() {
        let pipe = format!(r"\\.\pipe\dasidm-test-{}", std::process::id());
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap();
        let seen = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let s = seen.clone();
        let handler: Handler = Arc::new(move |req| match req {
            Request::Ping => json!({ "ok": true, "version": "test" }),
            Request::Download(d) => {
                s.lock().push(d.url.clone());
                json!({ "accepted": true })
            }
        });
        let p = pipe.clone();
        rt.spawn(async move { serve(p, handler).await.unwrap() });
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(relay(&pipe, &json!({ "type": "ping" }), false).unwrap()["version"], "test");
        let r = relay(&pipe, &json!({ "type": "download", "url": "https://example.org/big.iso", "fileName": "big.iso" }), false).unwrap();
        assert_eq!(r["accepted"], true);
        // Several at once (the server opens a new instance for each).
        let threads: Vec<_> = (0..4)
            .map(|i| {
                let pipe = pipe.clone();
                std::thread::spawn(move || relay(&pipe, &json!({ "type": "download", "url": format!("https://example.org/{i}") }), false).unwrap())
            })
            .collect();
        for t in threads {
            assert_eq!(t.join().unwrap()["accepted"], true);
        }
        assert_eq!(seen.lock().len(), 5);
        assert_eq!(relay(&pipe, &json!({ "type": "nope" }), false).unwrap()["ok"], false);
        // Nothing listening: a clear error, not a hang.
        let t = Instant::now();
        assert!(relay(r"\\.\pipe\dasidm-nobody-here", &json!({ "type": "ping" }), false).is_err());
        assert!(t.elapsed() < Duration::from_secs(10));
        rt.shutdown_background();
    }
}
