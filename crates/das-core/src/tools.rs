//! Helpers for video and music links: yt-dlp (reads the site), ffmpeg (joins and converts)
//! and deno (yt-dlp needs a JavaScript runtime for full YouTube support).
//!
//! They are downloaded only after the user agrees, from the publishers' own GitHub releases,
//! and every file is checked against the publisher's SHA-256 list before it is used.

use crate::model::CoreEvent;
use crate::manager::Emit;
use das_engine::{DownloadRequest, Engine};
use parking_lot::Mutex;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// One helper: where it comes from and how to check it.
struct Source {
    name: &'static str,
    /// Folder under the tools directory.
    dir: &'static str,
    url: &'static str,
    sums_url: &'static str,
    /// File name as written in the checksum list.
    sums_name: &'static str,
    /// Rough download size, shown before the user agrees.
    approx_size: u64,
    /// Which files to keep from the zip (by the last path components).
    keep: fn(&str) -> Option<String>,
}

const MB: u64 = 1_000_000;

const SOURCES: [Source; 3] = [
    Source {
        name: "yt-dlp",
        dir: "yt-dlp",
        url: "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_win.zip",
        sums_url: "https://github.com/yt-dlp/yt-dlp/releases/latest/download/SHA2-256SUMS",
        sums_name: "yt-dlp_win.zip",
        approx_size: 24 * MB,
        // The onedir build: yt-dlp.exe plus its _internal folder. It starts much faster than
        // the single-file exe, which unpacks itself on every run.
        keep: |path| {
            let p = path.replace('\\', "/");
            if let Some(i) = p.find("_internal/") {
                return Some(p[i..].to_string());
            }
            (p.rsplit('/').next() == Some("yt-dlp.exe")).then(|| "yt-dlp.exe".to_string())
        },
    },
    Source {
        name: "ffmpeg",
        dir: "ffmpeg",
        url: "https://github.com/yt-dlp/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl-shared.zip",
        sums_url: "https://github.com/yt-dlp/FFmpeg-Builds/releases/download/latest/checksums.sha256",
        sums_name: "ffmpeg-master-latest-win64-gpl-shared.zip",
        approx_size: 91 * MB,
        // bin/ffmpeg.exe, bin/ffprobe.exe and their DLLs; ffplay is not needed.
        keep: |path| {
            let p = path.replace('\\', "/");
            let (dir, file) = p.rsplit_once('/')?;
            (dir.ends_with("/bin") && !file.is_empty() && !file.starts_with("ffplay")).then(|| file.to_string())
        },
    },
    Source {
        name: "deno",
        dir: "deno",
        url: "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-pc-windows-msvc.zip",
        sums_url: "https://github.com/denoland/deno/releases/latest/download/deno-x86_64-pc-windows-msvc.zip.sha256sum",
        sums_name: "deno-x86_64-pc-windows-msvc.zip",
        approx_size: 43 * MB,
        keep: |path| (path.rsplit(['/', '\\']).next() == Some("deno.exe")).then(|| "deno.exe".to_string()),
    },
];

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsStatus {
    /// Everything needed for video and music links is in place.
    pub ready: bool,
    pub yt_dlp: Option<String>,
    pub ffmpeg: bool,
    pub deno: bool,
    /// Roughly how much a first install downloads.
    pub install_size: u64,
    pub installing: Option<InstallProgress>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallProgress {
    pub step: String,
    pub index: usize,
    pub count: usize,
    pub downloaded: u64,
    pub size: Option<u64>,
}

pub struct Tools {
    dir: PathBuf,
    emit: Emit,
    busy: AtomicBool,
    progress: Mutex<Option<InstallProgress>>,
    error: Mutex<Option<String>>,
}

impl Tools {
    pub fn new(emit: Emit) -> Arc<Self> {
        let dir = std::env::var_os("DASIDM_TOOLS_DIR").map(PathBuf::from).unwrap_or_else(|| {
            dirs::data_local_dir().unwrap_or_else(std::env::temp_dir).join("DasIDM").join("tools")
        });
        Arc::new(Self { dir, emit, busy: AtomicBool::new(false), progress: Mutex::new(None), error: Mutex::new(None) })
    }

    pub fn yt_dlp(&self) -> PathBuf {
        self.dir.join("yt-dlp").join("yt-dlp.exe")
    }
    pub fn ffmpeg_dir(&self) -> PathBuf {
        self.dir.join("ffmpeg")
    }
    pub fn ffmpeg(&self) -> PathBuf {
        self.ffmpeg_dir().join("ffmpeg.exe")
    }
    pub fn ffprobe(&self) -> PathBuf {
        self.ffmpeg_dir().join("ffprobe.exe")
    }
    /// What the audio converter needs (ffmpeg alone).
    pub fn ffmpeg_ready(&self) -> bool {
        self.ffmpeg().is_file() && self.ffprobe().is_file()
    }
    pub fn deno(&self) -> PathBuf {
        self.dir.join("deno").join("deno.exe")
    }

    pub fn ready(&self) -> bool {
        self.yt_dlp().is_file() && self.ffmpeg_dir().join("ffmpeg.exe").is_file() && self.deno().is_file()
    }

    pub fn status(&self) -> ToolsStatus {
        let version = std::fs::read_to_string(self.dir.join("yt-dlp").join("version.txt")).ok().map(|v| v.trim().to_string());
        let yt = self.yt_dlp().is_file();
        let ffmpeg = self.ffmpeg_dir().join("ffmpeg.exe").is_file();
        let deno = self.deno().is_file();
        let missing: u64 = [yt, ffmpeg, deno].iter().zip(SOURCES.iter()).filter(|(have, _)| !**have).map(|(_, s)| s.approx_size).sum();
        ToolsStatus {
            ready: yt && ffmpeg && deno,
            yt_dlp: yt.then(|| version.unwrap_or_else(|| "installed".into())),
            ffmpeg,
            deno,
            install_size: missing,
            installing: self.progress.lock().clone(),
            error: self.error.lock().clone(),
        }
    }

    fn send(&self) {
        (self.emit)(CoreEvent::Tools { status: self.status() });
    }

    /// Download whatever is missing (or everything, with `force`). Progress goes out as
    /// `CoreEvent::Tools`. Returns once finished; a second call while busy returns at once.
    pub async fn install(self: &Arc<Self>, engine: &Engine, force: bool) -> Result<(), String> {
        if self.busy.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        *self.error.lock() = None;
        let result = self.install_inner(engine, force).await;
        *self.progress.lock() = None;
        if let Err(e) = &result {
            *self.error.lock() = Some(e.clone());
        }
        self.busy.store(false, Ordering::SeqCst);
        self.send();
        result
    }

    async fn install_inner(self: &Arc<Self>, engine: &Engine, force: bool) -> Result<(), String> {
        let todo: Vec<&Source> = SOURCES
            .iter()
            .filter(|s| {
                force
                    || match s.name {
                        "yt-dlp" => !self.yt_dlp().is_file(),
                        "ffmpeg" => !self.ffmpeg_dir().join("ffmpeg.exe").is_file(),
                        _ => !self.deno().is_file(),
                    }
            })
            .collect();
        let staging = self.dir.join(".staging");
        for (index, src) in todo.iter().enumerate() {
            *self.progress.lock() =
                Some(InstallProgress { step: src.name.into(), index, count: todo.len(), downloaded: 0, size: Some(src.approx_size) });
            self.send();
            let (sums, _) = engine
                .fetch_small(src.sums_url, 256 * 1024)
                .await
                .map_err(|e| format!("Could not read the {} checksum list: {e}", src.name))?;
            let expected = expected_sha256(&String::from_utf8_lossy(&sums), src.sums_name)
                .ok_or_else(|| format!("The {} checksum list has no entry for {}", src.name, src.sums_name))?;

            let _ = tokio::fs::remove_dir_all(&staging).await;
            tokio::fs::create_dir_all(&staging).await.map_err(|e| e.to_string())?;
            let zip = self.fetch(engine, src, &staging, index, todo.len()).await?;

            let got = {
                let zip = zip.clone();
                tokio::task::spawn_blocking(move || sha256_file(&zip)).await.map_err(|e| e.to_string())?.map_err(|e| e.to_string())?
            };
            if !got.eq_ignore_ascii_case(&expected) {
                let _ = tokio::fs::remove_dir_all(&staging).await;
                return Err(format!("{} failed its checksum check, so it was not installed", src.name));
            }

            // Unpack next to the old copy, then swap, so a failure never leaves a broken tool.
            let target = self.dir.join(src.dir);
            let fresh = self.dir.join(format!("{}.new", src.dir));
            let keep = src.keep;
            {
                let (zip, fresh) = (zip.clone(), fresh.clone());
                tokio::task::spawn_blocking(move || unpack(&zip, &fresh, keep))
                    .await
                    .map_err(|e| e.to_string())?
                    .map_err(|e| format!("Could not unpack {}: {e}", src.name))?;
            }
            let _ = std::fs::remove_dir_all(&target);
            std::fs::rename(&fresh, &target).map_err(|e| format!("Could not install {}: {e}", src.name))?;
            if src.name == "yt-dlp" {
                let _ = std::fs::write(target.join("zip.sha256"), &got);
                if let Some(v) = self.version().await {
                    let _ = std::fs::write(target.join("version.txt"), v);
                }
            }
        }
        let _ = tokio::fs::remove_dir_all(&staging).await;
        let _ = std::fs::write(self.dir.join("checked.txt"), crate::model::now_ms().to_string());
        Ok(())
    }

    async fn fetch(&self, engine: &Engine, src: &Source, dir: &Path, index: usize, count: usize) -> Result<PathBuf, String> {
        let req = DownloadRequest {
            url: src.url.into(),
            headers: vec![],
            save_dir: dir.to_path_buf(),
            file_name: Some(src.sums_name.into()),
            connections: 8,
            allow_html: false,
        };
        let handle = engine.start(req, None, vec![]);
        let waiter = handle.clone();
        let done = async move { waiter.wait().await };
        tokio::pin!(done);
        let mut tick = tokio::time::interval(Duration::from_millis(250));
        let result = loop {
            tokio::select! {
                r = &mut done => break r,
                _ = tick.tick() => {
                    let s = handle.snapshot();
                    *self.progress.lock() = Some(InstallProgress {
                        step: src.name.into(), index, count, downloaded: s.downloaded, size: s.size.or(Some(src.approx_size)),
                    });
                    self.send();
                }
            }
        };
        result.map(|c| c.path).map_err(|e| format!("Could not download {}: {e}", src.name))
    }

    /// `yt-dlp --version`.
    async fn version(&self) -> Option<String> {
        let out = hidden(&self.yt_dlp()).arg("--version").output().await.ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    /// Sites change often, so yt-dlp is refreshed at most once a week, in the background.
    /// Only yt-dlp is checked; ffmpeg and deno rarely matter for compatibility.
    pub async fn update_if_stale(self: &Arc<Self>, engine: &Engine) {
        const WEEK_MS: i64 = 7 * 24 * 3600 * 1000;
        let checked = std::fs::read_to_string(self.dir.join("checked.txt")).ok().and_then(|s| s.trim().parse::<i64>().ok());
        if !self.ready() || checked.is_some_and(|t| crate::model::now_ms() - t < WEEK_MS) {
            return;
        }
        let _ = std::fs::write(self.dir.join("checked.txt"), crate::model::now_ms().to_string());
        let _ = self.update(engine).await;
    }

    /// Reinstall yt-dlp if the publisher has a newer build. Returns whether it changed.
    pub async fn update(self: &Arc<Self>, engine: &Engine) -> Result<bool, String> {
        let src = &SOURCES[0];
        let (sums, _) = engine.fetch_small(src.sums_url, 256 * 1024).await.map_err(|e| e.to_string())?;
        let latest = expected_sha256(&String::from_utf8_lossy(&sums), src.sums_name).ok_or("No checksum entry")?;
        let installed = std::fs::read_to_string(self.dir.join("yt-dlp").join("zip.sha256")).unwrap_or_default();
        if installed.trim().eq_ignore_ascii_case(&latest) && self.yt_dlp().is_file() {
            return Ok(false);
        }
        let _ = std::fs::remove_dir_all(self.dir.join("yt-dlp"));
        self.install(engine, false).await.map(|_| true)
    }
}

/// A console program run without flashing a console window.
pub fn hidden(program: &Path) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    cmd
}

/// Find the SHA-256 for `name` in a checksum list. Understands `HASH  name` lines and
/// PowerShell's `Get-FileHash` format (`Hash : HASH`), which lists a single file.
pub fn expected_sha256(list: &str, name: &str) -> Option<String> {
    let is_hash = |t: &str| t.len() == 64 && t.chars().all(|c| c.is_ascii_hexdigit());
    let mut listed = false;
    for line in list.lines() {
        let mut parts = line.split_whitespace();
        if let (Some(h), Some(n)) = (parts.next(), parts.next()) {
            if is_hash(h) {
                listed = true;
                if n.trim_start_matches('*') == name {
                    return Some(h.to_ascii_lowercase());
                }
            }
        }
    }
    if listed {
        return None; // a multi-file list without our file
    }
    // Single-file formats: the only hash in the text.
    let hashes: Vec<&str> = list.split(|c: char| !c.is_ascii_hexdigit()).filter(|t| is_hash(t)).collect();
    (hashes.len() == 1).then(|| hashes[0].to_ascii_lowercase())
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn unpack(zip_path: &Path, to: &Path, keep: fn(&str) -> Option<String>) -> Result<(), String> {
    let _ = std::fs::remove_dir_all(to);
    std::fs::create_dir_all(to).map_err(|e| e.to_string())?;
    let file = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut wrote = 0;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        if entry.is_dir() {
            continue;
        }
        // enclosed_name() rejects absolute paths and `..` (zip-slip).
        let Some(name) = entry.enclosed_name() else { continue };
        let Some(rel) = keep(&name.to_string_lossy()) else { continue };
        if rel.split('/').any(|c| c == ".." || c.is_empty()) {
            continue;
        }
        let out = to.join(&rel);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut f = std::fs::File::create(&out).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut f).map_err(|e| e.to_string())?;
        wrote += 1;
    }
    if wrote == 0 {
        return Err("the archive did not contain the expected files".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_checksum_lists() {
        let ytdlp = "e51e69e89489f199530f13d32a4342da3ddb2cf34842f364603a066db05afbf3  yt-dlp\n\
                     fd51b212cf8380c7a52ee08c0d6be7f3c59b2f2c832eaf7b45acfae6eab57419  yt-dlp_win.zip\n";
        assert_eq!(
            expected_sha256(ytdlp, "yt-dlp_win.zip").as_deref(),
            Some("fd51b212cf8380c7a52ee08c0d6be7f3c59b2f2c832eaf7b45acfae6eab57419")
        );
        assert_eq!(expected_sha256(ytdlp, "yt-dlp.exe"), None);
        let deno = "\nAlgorithm : SHA256\nHash      : CBA84B26E0DF5AE7F74FC66B86A95B866872A86292EBD84FA1E959927EDD93BF\nPath      : C:\\a\\deno.zip\n";
        assert_eq!(
            expected_sha256(deno, "deno-x86_64-pc-windows-msvc.zip").as_deref(),
            Some("cba84b26e0df5ae7f74fc66b86a95b866872a86292ebd84fa1e959927edd93bf")
        );
    }

    #[test]
    fn keeps_the_right_files() {
        let yt = SOURCES[0].keep;
        assert_eq!(yt("yt-dlp.exe").as_deref(), Some("yt-dlp.exe"));
        assert_eq!(yt("_internal/python3.dll").as_deref(), Some("_internal/python3.dll"));
        assert_eq!(yt("yt-dlp_win/_internal/a/b.pyd").as_deref(), Some("_internal/a/b.pyd"));
        let ff = SOURCES[1].keep;
        assert_eq!(ff("ffmpeg-master/bin/ffmpeg.exe").as_deref(), Some("ffmpeg.exe"));
        assert_eq!(ff("ffmpeg-master/bin/avcodec-62.dll").as_deref(), Some("avcodec-62.dll"));
        assert_eq!(ff("ffmpeg-master/bin/ffplay.exe"), None);
        assert_eq!(ff("ffmpeg-master/doc/x.html"), None);
        let de = SOURCES[2].keep;
        assert_eq!(de("deno.exe").as_deref(), Some("deno.exe"));
    }
}
