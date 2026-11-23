//! Video and music links (YouTube and the other sites yt-dlp understands).
//!
//! `resolve` reads a link with `yt-dlp -J` and turns the site's long format list into the
//! few choices people make (a resolution, or an audio format). `VideoJob` runs the chosen
//! download as a yt-dlp process and reports progress parsed from its output.

use crate::tools::{Tools, hidden};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Choice {
    /// Up to `height` lines. `compat` = MP4 with H.264 video and AAC audio, which plays everywhere.
    Video { height: u32, compat: bool },
    Audio { format: AudioFormat },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    M4a,
    Mp3,
    Opus,
    Flac,
}

impl AudioFormat {
    fn ext(self) -> &'static str {
        match self {
            Self::M4a => "m4a",
            Self::Mp3 => "mp3",
            Self::Opus => "opus",
            Self::Flac => "flac",
        }
    }
}

/// One row of the quality picker.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoOption {
    pub choice: Choice,
    /// "1080p", "MP3".
    pub label: String,
    /// "MP4 · H.264 · 60 fps · plays everywhere".
    pub detail: String,
    /// The file's extension once finished.
    pub ext: String,
    pub size: Option<u64>,
    /// The size is an estimate.
    #[serde(default)]
    pub approx: bool,
    /// yt-dlp `-f` value: exact format ids first, then a generic fallback.
    pub selector: String,
    /// Something people should know before choosing (e.g. FLAC from a lossy source).
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub recommended: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoInfo {
    pub url: String,
    pub title: String,
    pub channel: Option<String>,
    /// Seconds.
    pub duration: Option<u64>,
    pub thumbnail: Option<String>,
    /// The site, as yt-dlp names it ("Youtube", "Vimeo").
    pub site: String,
    pub is_live: bool,
    pub options: Vec<VideoOption>,
    /// The link belongs to (or is) a playlist.
    pub playlist: Option<PlaylistInfo>,
    /// The link is only a playlist, with no single video to pick.
    pub playlist_only: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistInfo {
    pub title: String,
    pub count: usize,
}

/// One video of a playlist.
#[derive(Clone, Debug)]
pub struct Entry {
    pub url: String,
    pub title: String,
    pub duration: Option<u64>,
    pub thumbnail: Option<String>,
}

/// Sites whose links are video or music pages almost always (kept in step with the Add sheet).
pub fn is_video_site(link: &str) -> bool {
    const HOSTS: [&str; 18] = [
        "youtube.com", "youtu.be", "youtube-nocookie.com", "vimeo.com", "soundcloud.com", "bandcamp.com", "dailymotion.com",
        "twitch.tv", "tiktok.com", "instagram.com", "x.com", "twitter.com", "reddit.com", "facebook.com", "bilibili.com",
        "rumble.com", "odysee.com", "mixcloud.com",
    ];
    url::Url::parse(link).ok().and_then(|u| u.host_str().map(str::to_ascii_lowercase)).is_some_and(|h| {
        HOSTS.iter().any(|d| h == *d || h.ends_with(&format!(".{d}")))
    })
}

// ---------------------------------------------------------------------------------------
// Reading a link

fn base_args(tools: &Tools) -> Vec<String> {
    vec![
        "--ffmpeg-location".into(),
        tools.ffmpeg_dir().to_string_lossy().into_owned(),
        "--js-runtimes".into(),
        format!("deno:{}", tools.deno().to_string_lossy()),
        "--no-warnings".into(),
    ]
}

async fn json_of(tools: &Tools, args: &[&str], url: &str, timeout: Duration) -> Result<Value, String> {
    let mut cmd = hidden(&tools.yt_dlp());
    cmd.args(base_args(tools)).args(args).arg("--").arg(url).stdout(Stdio::piped()).stderr(Stdio::piped());
    let out = tokio::time::timeout(timeout, cmd.output())
        .await
        .map_err(|_| "The site took too long to answer".to_string())?
        .map_err(|e| format!("Could not run yt-dlp: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(friendly_error(err.lines().filter(|l| l.starts_with("ERROR")).last().unwrap_or(&err)));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("yt-dlp gave an unreadable answer: {e}"))
}

/// Read a video page: title, length, thumbnail and the quality choices.
pub async fn resolve(tools: &Tools, url: &str) -> Result<VideoInfo, String> {
    let main = json_of(tools, &["-J", "--flat-playlist", "--no-playlist"], url, Duration::from_secs(60));
    // A watch link inside a playlist: count the playlist too, at the same time.
    let in_list = url::Url::parse(url).is_ok_and(|u| u.query_pairs().any(|(k, _)| k == "list"));
    let (info, list) = if in_list {
        let list = json_of(tools, &["-J", "--flat-playlist", "--yes-playlist"], url, Duration::from_secs(60));
        let (a, b) = tokio::join!(main, list);
        (a?, b.ok())
    } else {
        (main.await?, None)
    };

    let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    if info.get("_type").and_then(Value::as_str) == Some("playlist") {
        let count = info.get("entries").and_then(Value::as_array).map_or(0, Vec::len);
        return Ok(VideoInfo {
            url: s(&info, "webpage_url").unwrap_or_else(|| url.into()),
            title: s(&info, "title").unwrap_or_else(|| "Playlist".into()),
            channel: s(&info, "channel").or_else(|| s(&info, "uploader")),
            duration: None,
            thumbnail: pick_thumbnail(&info),
            site: site_name(&s(&info, "extractor_key").unwrap_or_default()),
            is_live: false,
            options: generic_options(),
            playlist: Some(PlaylistInfo { title: s(&info, "title").unwrap_or_default(), count }),
            playlist_only: true,
        });
    }
    let mut v = info_from_json(&info);
    v.url = s(&info, "webpage_url").unwrap_or_else(|| url.into());
    if let Some(list) = list.filter(|l| l.get("_type").and_then(Value::as_str) == Some("playlist")) {
        let count = list.get("entries").and_then(Value::as_array).map_or(0, Vec::len);
        if count > 1 {
            v.playlist = Some(PlaylistInfo { title: s(&list, "title").unwrap_or_default(), count });
        }
    }
    Ok(v)
}

/// The videos of a playlist link, in order.
pub async fn playlist_entries(tools: &Tools, url: &str) -> Result<Vec<Entry>, String> {
    let list = json_of(tools, &["-J", "--flat-playlist", "--yes-playlist"], url, Duration::from_secs(90)).await?;
    let entries = list.get("entries").and_then(Value::as_array).cloned().unwrap_or_default();
    Ok(entries
        .iter()
        .filter_map(|e| {
            let url = e.get("url").or_else(|| e.get("webpage_url")).and_then(Value::as_str)?.to_string();
            Some(Entry {
                url,
                title: e.get("title").and_then(Value::as_str).unwrap_or("Video").to_string(),
                duration: e.get("duration").and_then(Value::as_f64).map(|d| d.round() as u64),
                thumbnail: pick_thumbnail(e),
            })
        })
        .collect())
}

/// Turn a single video's `yt-dlp -J` output into what the picker shows.
pub fn info_from_json(info: &Value) -> VideoInfo {
    let s = |k: &str| info.get(k).and_then(Value::as_str).map(str::to_string);
    let duration = info.get("duration").and_then(Value::as_f64).map(|d| d.round() as u64);
    VideoInfo {
        url: s("webpage_url").unwrap_or_default(),
        title: s("title").unwrap_or_else(|| "Video".into()),
        channel: s("channel").or_else(|| s("uploader")),
        duration,
        thumbnail: pick_thumbnail(info),
        site: site_name(&s("extractor_key").unwrap_or_default()),
        is_live: info.get("is_live").and_then(Value::as_bool).unwrap_or(false),
        options: options_from(info, duration),
        playlist: None,
        playlist_only: false,
    }
}

/// Site names as people write them (yt-dlp's extractor keys are "Youtube", "Soundcloud"…).
fn site_name(key: &str) -> String {
    match key.to_ascii_lowercase().as_str() {
        k if k.starts_with("youtube") => "YouTube".into(),
        "soundcloud" => "SoundCloud".into(),
        "tiktok" => "TikTok".into(),
        "twitter" => "X".into(),
        "bandcamp" => "Bandcamp".into(),
        "dailymotion" => "Dailymotion".into(),
        _ => key.to_string(),
    }
}

/// A small 16:9 thumbnail (the pass shows it ~350 px wide), not the 1280 px original.
fn pick_thumbnail(info: &Value) -> Option<String> {
    let list = info.get("thumbnails").and_then(Value::as_array);
    let mut best: Option<(i64, String)> = None;
    for t in list.into_iter().flatten() {
        let (Some(url), Some(w), Some(h)) =
            (t.get("url").and_then(Value::as_str), t.get("width").and_then(Value::as_i64), t.get("height").and_then(Value::as_i64))
        else {
            continue;
        };
        if url.contains(".webp") || h == 0 || (w as f64 / h as f64 - 16.0 / 9.0).abs() > 0.1 || !(320..=720).contains(&w) {
            continue;
        }
        if best.as_ref().is_none_or(|(bw, _)| w > *bw) {
            best = Some((w, url.to_string()));
        }
    }
    best.map(|(_, u)| u).or_else(|| info.get("thumbnail").and_then(Value::as_str).map(str::to_string))
}

#[derive(Clone, Debug)]
struct Fmt {
    id: String,
    ext: String,
    vcodec: Option<String>,
    acodec: Option<String>,
    height: u32,
    width: u32,
    fps: f64,
    hdr: bool,
    size: Option<u64>,
    approx: bool,
    tbr: f64,
    abr: f64,
    lang_pref: i64,
}

impl Fmt {
    fn has_video(&self) -> bool {
        self.vcodec.as_deref().map_or(self.height > 0, |v| v != "none")
    }
    fn has_audio(&self) -> bool {
        self.acodec.as_deref().is_none_or(|a| a != "none")
    }
    /// The resolution people call it by: 1920x800 and 1080x1920 are both "1080p".
    fn lines(&self) -> u32 {
        if self.width == 0 {
            return self.height;
        }
        let (short, long) = (self.width.min(self.height), self.width.max(self.height));
        short.max(long * 9 / 16)
    }
    fn is_h264(&self) -> bool {
        self.vcodec.as_deref().is_some_and(|v| v.starts_with("avc1") || v.starts_with("h264"))
    }
    fn codec_name(&self) -> String {
        let v = self.vcodec.as_deref().unwrap_or("");
        match () {
            _ if v.starts_with("avc1") || v.starts_with("h264") => "H.264".into(),
            _ if v.starts_with("vp09") || v.starts_with("vp9") => "VP9".into(),
            _ if v.starts_with("av01") => "AV1".into(),
            _ if v.starts_with("hev1") || v.starts_with("hvc1") || v.starts_with("h265") => "H.265".into(),
            _ => v.split('.').next().unwrap_or("").to_uppercase(),
        }
    }
}

fn parse_formats(info: &Value, duration: Option<u64>) -> Vec<Fmt> {
    let list = info.get("formats").and_then(Value::as_array).cloned().unwrap_or_default();
    list.iter()
        .filter_map(|f| {
            let s = |k: &str| f.get(k).and_then(Value::as_str).map(str::to_string);
            let n = |k: &str| f.get(k).and_then(Value::as_f64);
            let id = s("format_id")?;
            let ext = s("ext").unwrap_or_default();
            let note = s("format_note").unwrap_or_default().to_lowercase();
            if ext == "mhtml" || note.contains("storyboard") || f.get("has_drm").and_then(Value::as_bool) == Some(true) {
                return None;
            }
            // YouTube's "-drc" audio is a dynamic-range-compressed duplicate.
            if id.contains("-drc") {
                return None;
            }
            let tbr = n("tbr").unwrap_or(0.0);
            let (size, approx) = match (n("filesize"), n("filesize_approx")) {
                (Some(x), _) => (Some(x as u64), false),
                (None, Some(x)) => (Some(x as u64), true),
                _ => (duration.filter(|_| tbr > 0.0).map(|d| (tbr * 125.0 * d as f64) as u64), true),
            };
            Some(Fmt {
                id,
                ext,
                vcodec: s("vcodec"),
                acodec: s("acodec"),
                height: n("height").unwrap_or(0.0) as u32,
                width: n("width").unwrap_or(0.0) as u32,
                fps: n("fps").unwrap_or(0.0),
                hdr: s("dynamic_range").is_some_and(|d| d != "SDR"),
                size,
                approx,
                tbr,
                abr: n("abr").unwrap_or(tbr),
                lang_pref: f.get("language_preference").and_then(Value::as_i64).unwrap_or(0),
            })
        })
        .collect()
}

const STEPS: [u32; 9] = [4320, 2160, 1440, 1080, 720, 480, 360, 240, 144];

fn step_of(lines: u32) -> Option<u32> {
    STEPS.iter().copied().find(|s| lines as f64 >= *s as f64 * 0.95)
}

fn add_sizes(parts: &[&Fmt]) -> (Option<u64>, bool) {
    let mut total = 0;
    let mut approx = false;
    for p in parts {
        match p.size {
            Some(s) => total += s,
            None => return (None, true),
        }
        approx |= p.approx;
    }
    (Some(total), approx)
}

pub fn generic_selector(choice: &Choice) -> String {
    match choice {
        Choice::Video { height: h, compat: true } => {
            format!("bv*[height<={h}][vcodec^=avc1]+ba[ext=m4a]/b[height<={h}][ext=mp4]/bv*[height<={h}]+ba/b[height<={h}]/b")
        }
        Choice::Video { height: h, compat: false } => format!("bv*[height<={h}]+ba/b[height<={h}]/b"),
        Choice::Audio { format: AudioFormat::M4a } => "ba[ext=m4a]/ba/b".into(),
        Choice::Audio { format: AudioFormat::Opus } => "ba[acodec=opus]/ba/b".into(),
        Choice::Audio { .. } => "ba/b".into(),
    }
}

fn fps_text(f: &Fmt) -> String {
    if f.fps > 31.0 { format!(" · {} fps", f.fps.round()) } else { String::new() }
}

fn audio_options(src: Option<&Fmt>, m4a: Option<&Fmt>, best: Option<&Fmt>, duration: Option<u64>) -> Vec<VideoOption> {
    let Some(src) = src else { return vec![] };
    let mut out = Vec::new();
    let kbps = |f: &Fmt| if f.abr > 0.0 { format!(" {} kbps", f.abr.round()) } else { String::new() };
    let mk = |format: AudioFormat, exact: &str, label: &str, detail: String, size: (Option<u64>, bool), note: Option<&str>| {
        let choice = Choice::Audio { format };
        VideoOption {
            selector: format!("{exact}/{}", generic_selector(&choice)),
            choice,
            label: label.into(),
            detail,
            ext: format.ext().into(),
            size: size.0,
            approx: size.1,
            note: note.map(str::to_string),
            recommended: false,
        }
    };
    if let Some(a) = m4a {
        out.push(mk(AudioFormat::M4a, &a.id, "M4A", format!("AAC{} · original, not re-encoded", kbps(a)), (a.size, a.approx), None));
    }
    out.push(mk(
        AudioFormat::Mp3,
        &src.id,
        "MP3",
        "320 kbps · converted, plays everywhere".into(),
        (duration.map(|d| d * 40_000), true),
        None,
    ));
    if let Some(o) = best.filter(|b| b.acodec.as_deref().is_some_and(|a| a.starts_with("opus"))) {
        out.push(mk(AudioFormat::Opus, &o.id, "Opus", format!("Opus{} · original, not re-encoded", kbps(o)), (o.size, o.approx), None));
    }
    out.push(mk(
        AudioFormat::Flac,
        &src.id,
        "FLAC",
        "Lossless container".into(),
        (duration.map(|d| d * 110_000), true),
        Some("The site only has lossy audio, so FLAC is a bigger file but does not sound better."),
    ));
    if let Some(first) = out.first_mut() {
        first.recommended = true;
    }
    out
}

/// Group the site's formats into the choices people make.
fn options_from(info: &Value, duration: Option<u64>) -> Vec<VideoOption> {
    let formats = parse_formats(info, duration);
    let by_audio = |a: &&Fmt, b: &&Fmt| (a.lang_pref, a.abr as i64).cmp(&(b.lang_pref, b.abr as i64));
    let audio: Vec<&Fmt> = formats.iter().filter(|f| !f.has_video() && f.has_audio()).collect();
    let best_audio = audio.iter().copied().max_by(by_audio);
    let m4a = audio.iter().copied().filter(|f| f.ext == "m4a").max_by(by_audio);
    let video: Vec<&Fmt> = formats.iter().filter(|f| f.has_video()).collect();

    // Best first: higher frame rate, then SDR (HDR looks washed out on most screens), then the
    // newer codec (as yt-dlp ranks them; bitrates are not comparable across codecs), then bitrate.
    let codec_rank = |f: &Fmt| match f.codec_name().as_str() {
        "AV1" => 3,
        "VP9" => 2,
        "H.265" => 2,
        "H.264" => 1,
        _ => 0,
    };
    let rank = |f: &&Fmt| ((f.fps.round() as i64), !f.hdr, codec_rank(f), (f.tbr * 10.0) as i64);
    let mut out = Vec::new();
    for step in STEPS {
        let here: Vec<&Fmt> = video.iter().copied().filter(|f| step_of(f.lines()) == Some(step)).collect();
        if here.is_empty() {
            continue;
        }
        let pick = |compat: bool| -> Option<(Vec<&Fmt>, String)> {
            let best = here
                .iter()
                .copied()
                .filter(|f| !compat || (f.is_h264() && f.ext == "mp4"))
                .filter(|f| f.has_audio() || if compat { m4a.is_some() } else { best_audio.is_some() })
                .max_by(|a, b| rank(a).cmp(&rank(b)))?;
            if best.has_audio() {
                return Some((vec![best], best.ext.clone()));
            }
            let a = if compat { m4a? } else { best_audio? };
            let ext = match (best.ext.as_str(), a.ext.as_str()) {
                ("mp4", "m4a") => "mp4",
                ("webm", "webm") => "webm",
                _ => "mkv",
            };
            Some((vec![best, a], ext.into()))
        };
        let label = format!("{step}p");
        let tag = match step {
            4320 => " · 8K",
            2160 => " · 4K",
            _ => "",
        };
        let compat = pick(true);
        let best = pick(false);
        let same = matches!((&compat, &best), (Some((c, _)), Some((b, _))) if c.iter().map(|f| &f.id).eq(b.iter().map(|f| &f.id)));
        if let Some((parts, ext)) = &best {
            if !same {
                let choice = Choice::Video { height: step, compat: false };
                let ids: Vec<&str> = parts.iter().map(|f| f.id.as_str()).collect();
                let (size, approx) = add_sizes(parts);
                out.push(VideoOption {
                    selector: format!("{}/{}", ids.join("+"), generic_selector(&choice)),
                    choice,
                    label: label.clone(),
                    detail: format!("{} · {}{}{} · best quality", ext.to_uppercase(), parts[0].codec_name(), fps_text(parts[0]), tag),
                    ext: ext.clone(),
                    size,
                    approx,
                    note: None,
                    recommended: false,
                });
            }
        }
        if let Some((parts, _)) = &compat {
            let choice = Choice::Video { height: step, compat: true };
            let ids: Vec<&str> = parts.iter().map(|f| f.id.as_str()).collect();
            let (size, approx) = add_sizes(parts);
            out.push(VideoOption {
                selector: format!("{}/{}", ids.join("+"), generic_selector(&choice)),
                choice,
                label: label.clone(),
                detail: format!("MP4 · H.264{}{} · plays everywhere", fps_text(parts[0]), tag),
                ext: "mp4".into(),
                size,
                approx,
                note: None,
                recommended: false,
            });
        }
    }
    // Recommend the sharpest MP4 that is not bigger than 1080p (what most people want).
    if let Some(o) = out.iter_mut().find(|o| matches!(o.choice, Choice::Video { height, compat: true } if height <= 1080)) {
        o.recommended = true;
    } else if let Some(o) = out.first_mut() {
        o.recommended = true;
    }
    // Audio is taken from the best audio-only stream, or from the smallest video when the site has none.
    let src = best_audio.or_else(|| video.iter().copied().filter(|f| f.has_audio()).min_by_key(|f| f.height));
    out.extend(audio_options(src, m4a, best_audio, duration));
    out
}

/// Choices for a whole playlist, where each video has its own formats.
fn generic_options() -> Vec<VideoOption> {
    let video = |height: u32, compat: bool, label: &str, detail: &str| {
        let choice = Choice::Video { height, compat };
        VideoOption {
            selector: generic_selector(&choice),
            choice,
            label: label.into(),
            detail: detail.into(),
            ext: if compat { "mp4" } else { "mkv" }.into(),
            size: None,
            approx: true,
            note: None,
            recommended: false,
        }
    };
    let mut out = vec![
        video(4320, false, "Best", "Each video's best quality, any format"),
        video(1080, true, "1080p", "MP4 · H.264 · plays everywhere"),
        video(720, true, "720p", "MP4 · H.264 · plays everywhere"),
        video(480, true, "480p", "MP4 · H.264 · plays everywhere"),
    ];
    out[1].recommended = true;
    for (format, label, detail) in [
        (AudioFormat::M4a, "M4A", "AAC · original, not re-encoded"),
        (AudioFormat::Mp3, "MP3", "320 kbps · converted, plays everywhere"),
        (AudioFormat::Opus, "Opus", "Opus · original, not re-encoded"),
        (AudioFormat::Flac, "FLAC", "Lossless container"),
    ] {
        let choice = Choice::Audio { format };
        out.push(VideoOption {
            selector: generic_selector(&choice),
            choice,
            label: label.into(),
            detail: detail.into(),
            ext: format.ext().into(),
            size: None,
            approx: true,
            note: (format == AudioFormat::Flac)
                .then(|| "The site only has lossy audio, so FLAC is a bigger file but does not sound better.".into()),
            recommended: format == AudioFormat::M4a,
        });
    }
    out
}

// ---------------------------------------------------------------------------------------
// Running a download

/// What a video item remembers so the download can be started again (resume, restart).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoSpec {
    pub choice: Choice,
    pub selector: String,
    pub ext: String,
    /// "1080p", "MP3".
    pub quality: String,
    pub title: String,
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub duration: Option<u64>,
    #[serde(default)]
    pub site: Option<String>,
    /// File name of the stored thumbnail (in the data folder), once fetched.
    #[serde(default)]
    pub thumb: Option<String>,
    /// Temporary files yt-dlp left behind (deleted on remove or restart).
    #[serde(default)]
    pub partials: Vec<String>,
    /// Automatic retries used since the last success or manual resume.
    #[serde(default)]
    pub retries: u32,
}

/// Errors that a fresh attempt usually gets past (expired links, a refused fragment, a dropped connection).
pub fn is_transient(friendly: &str) -> bool {
    friendly.contains("(403)") || friendly.starts_with("Network problem")
}

/// The yt-dlp command line for one download.
pub fn download_args(tools: &Tools, spec: &VideoSpec, url: &str, save_dir: &Path) -> Vec<String> {
    let mut a = base_args(tools);
    a.extend(["-f".into(), spec.selector.clone()]);
    match &spec.choice {
        Choice::Video { .. } => {
            a.extend(["--merge-output-format".into(), spec.ext.clone()]);
        }
        Choice::Audio { format } => {
            a.extend(["-x".into(), "--audio-format".into(), format.ext().into()]);
            if *format == AudioFormat::Mp3 {
                a.extend(["--audio-quality".into(), "320K".into()]);
            }
            // Cover art inside music files; players show it.
            a.push("--embed-thumbnail".into());
        }
    }
    a.extend(
        [
            "--embed-metadata",
            "--no-playlist",
            "--newline",
            "--progress",
            "--no-mtime",
            "--no-simulate",
            "--concurrent-fragments",
            "8",
            "-P",
        ]
        .map(String::from),
    );
    a.push(save_dir.to_string_lossy().into_owned());
    a.extend(
        [
            "-o",
            "%(title).150B.%(ext)s",
            "--progress-template",
            "download:DASP|%(info.format_id)s|%(progress.status)s|%(progress.downloaded_bytes)s|%(progress.total_bytes)s|%(progress.total_bytes_estimate)s|%(progress.speed)s|%(progress.eta)s|%(progress.tmpfilename)s",
            "--progress-template",
            "postprocess:DASQ|%(progress.postprocessor)s|%(progress.status)s",
            "--print",
            "after_move:DASF|%(filepath)s",
            "--",
        ]
        .map(String::from),
    );
    a.push(url.into());
    a
}

#[derive(Clone, Debug, PartialEq)]
pub enum Line {
    Progress { format: String, status: String, downloaded: u64, total: Option<u64>, speed: Option<f64>, eta: Option<u64>, tmp: String },
    Post { processor: String, status: String },
    Final(PathBuf),
    Other,
}

/// Parse one line of yt-dlp output produced by the templates in `download_args`.
pub fn parse_line(line: &str) -> Line {
    let line = line.trim_end_matches(['\r', '\n']);
    let num = |s: &str| s.trim().parse::<f64>().ok().filter(|x| x.is_finite() && *x >= 0.0);
    if let Some(rest) = line.strip_prefix("DASP|") {
        let f: Vec<&str> = rest.splitn(8, '|').collect();
        if f.len() == 8 {
            return Line::Progress {
                format: f[0].to_string(),
                status: f[1].to_string(),
                downloaded: num(f[2]).unwrap_or(0.0) as u64,
                total: num(f[3]).or(num(f[4])).map(|x| x as u64),
                speed: num(f[5]),
                eta: num(f[6]).map(|x| x as u64),
                tmp: f[7].to_string(),
            };
        }
    } else if let Some(rest) = line.strip_prefix("DASQ|") {
        if let Some((p, s)) = rest.split_once('|') {
            return Line::Post { processor: p.to_string(), status: s.to_string() };
        }
    } else if let Some(path) = line.strip_prefix("DASF|") {
        return Line::Final(PathBuf::from(path));
    }
    Line::Other
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VideoPhase {
    #[default]
    Starting,
    Downloading,
    Merging,
    Converting,
    Finishing,
}

#[derive(Clone, Debug, Default)]
pub struct VideoState {
    /// Per stream (video, audio): format id, bytes so far, total if known.
    pub streams: Vec<(String, u64, Option<u64>)>,
    pub speed: f64,
    pub phase: VideoPhase,
    pub file_path: Option<PathBuf>,
    pub partials: Vec<String>,
    errors: VecDeque<String>,
}

impl VideoState {
    pub fn downloaded(&self) -> u64 {
        self.streams.iter().map(|s| s.1).sum()
    }
    pub fn known_total(&self) -> u64 {
        self.streams.iter().map(|s| s.2.unwrap_or(s.1)).sum()
    }

    fn apply(&mut self, line: Line) {
        match line {
            Line::Progress { format, status, downloaded, total, speed, tmp, .. } => {
                self.phase = VideoPhase::Downloading;
                let i = match self.streams.iter().position(|s| s.0 == format) {
                    Some(i) => i,
                    None => {
                        self.streams.push((format, 0, None));
                        self.streams.len() - 1
                    }
                };
                let s = &mut self.streams[i];
                s.1 = downloaded;
                s.2 = total.or(s.2);
                if status == "finished" {
                    let t = s.1.max(s.2.unwrap_or(0));
                    s.1 = t;
                    s.2 = Some(t);
                }
                if let Some(sp) = speed {
                    self.speed = if self.speed == 0.0 { sp } else { self.speed * 0.6 + sp * 0.4 };
                }
                if !tmp.is_empty() && tmp != "NA" && !self.partials.contains(&tmp) {
                    self.partials.push(tmp);
                }
            }
            Line::Post { processor, status } => {
                self.speed = 0.0;
                if status != "finished" {
                    self.phase = match processor.as_str() {
                        "Merger" | "VideoRemuxer" | "VideoConvertor" => VideoPhase::Merging,
                        "ExtractAudio" => VideoPhase::Converting,
                        _ => VideoPhase::Finishing,
                    };
                }
            }
            Line::Final(p) => self.file_path = Some(p),
            Line::Other => {}
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VideoError {
    Paused,
    Cancelled,
    Failed(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stop {
    Pause,
    Cancel,
}

struct Shared {
    st: Mutex<VideoState>,
    stop: Mutex<Option<Stop>>,
    job: Mutex<Option<KillGroup>>,
    done: tokio::sync::watch::Sender<Option<Result<PathBuf, VideoError>>>,
}

/// A running yt-dlp download. Cheap to clone.
#[derive(Clone)]
pub struct VideoJob {
    s: Arc<Shared>,
}

impl VideoJob {
    /// Start yt-dlp. Must be called inside a tokio runtime.
    pub fn start(program: PathBuf, args: Vec<String>) -> Self {
        let (done, _) = tokio::sync::watch::channel(None);
        let s = Arc::new(Shared { st: Mutex::new(VideoState::default()), stop: Mutex::new(None), job: Mutex::new(None), done });
        let s2 = s.clone();
        tokio::spawn(async move {
            let r = run(&s2, &program, &args).await;
            s2.job.lock().take(); // closes the job object: nothing it started survives
            let _ = s2.done.send(Some(r));
        });
        Self { s }
    }

    pub fn state(&self) -> VideoState {
        self.s.st.lock().clone()
    }

    fn stop(&self, how: Stop) {
        *self.s.stop.lock() = Some(how);
        if let Some(job) = self.s.job.lock().as_ref() {
            job.kill();
        }
    }
    /// Stop now; yt-dlp continues from its partial files next time.
    pub fn pause(&self) {
        self.stop(Stop::Pause);
    }
    pub fn cancel(&self) {
        self.stop(Stop::Cancel);
    }

    pub async fn wait(&self) -> Result<PathBuf, VideoError> {
        let mut rx = self.s.done.subscribe();
        loop {
            if let Some(r) = rx.borrow_and_update().clone() {
                return r;
            }
            if rx.changed().await.is_err() {
                return Err(VideoError::Failed("The download stopped unexpectedly".into()));
            }
        }
    }
}

async fn run(s: &Arc<Shared>, program: &Path, args: &[String]) -> Result<PathBuf, VideoError> {
    let mut cmd = hidden(program);
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| VideoError::Failed(format!("Could not run yt-dlp: {e}")))?;
    // yt-dlp starts ffmpeg (and deno); a job object lets pause stop all of them at once.
    if let Some(job) = KillGroup::new() {
        if let Some(h) = child.raw_handle() {
            job.add(h);
        }
        *s.job.lock() = Some(job);
    }
    if s.stop.lock().is_some() {
        if let Some(j) = s.job.lock().as_ref() {
            j.kill();
        }
        let _ = child.start_kill();
    }
    let stderr = child.stderr.take().map(|e| {
        let s = s.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(e).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                if l.starts_with("ERROR") {
                    let mut st = s.st.lock();
                    st.errors.push_back(l);
                    if st.errors.len() > 8 {
                        st.errors.pop_front();
                    }
                }
            }
        })
    });
    if let Some(out) = child.stdout.take() {
        let mut lines = BufReader::new(out).lines();
        while let Ok(Some(l)) = lines.next_line().await {
            let parsed = parse_line(&l);
            if parsed != Line::Other {
                s.st.lock().apply(parsed);
            }
        }
    }
    let status = child.wait().await.map_err(|e| VideoError::Failed(e.to_string()))?;
    if let Some(t) = stderr {
        let _ = t.await;
    }
    match *s.stop.lock() {
        Some(Stop::Pause) => return Err(VideoError::Paused),
        Some(Stop::Cancel) => return Err(VideoError::Cancelled),
        None => {}
    }
    let st = s.st.lock();
    match (&st.file_path, status.success()) {
        (Some(p), true) => Ok(p.clone()),
        (None, true) => Err(VideoError::Failed("yt-dlp finished but reported no file".into())),
        _ => Err(VideoError::Failed(friendly_error(st.errors.back().map(String::as_str).unwrap_or("yt-dlp stopped with an error")))),
    }
}

/// Plain words for yt-dlp's errors.
pub fn friendly_error(raw: &str) -> String {
    let l = raw.to_lowercase();
    let known = [
        ("private video", "This video is private"),
        ("confirm your age", "This video is age-restricted and needs a signed-in account"),
        ("age-restricted", "This video is age-restricted and needs a signed-in account"),
        ("not a bot", "The site wants to confirm you're not a bot; try again later"),
        ("members-only", "This video is for channel members only"),
        ("join this channel", "This video is for channel members only"),
        ("video unavailable", "This video is unavailable or was removed"),
        ("has been removed", "This video is unavailable or was removed"),
        ("unsupported url", "This site isn't supported for video"),
        ("requested format is not available", "That quality is no longer offered; pick another"),
        ("http error 403", "The site refused the download (403); try again"),
        ("http error 404", "The video was not found (404)"),
        ("live event will begin", "This live stream hasn't started yet"),
        ("premieres in", "This video hasn't premiered yet"),
        ("getaddrinfo failed", "Network problem: can't reach the site"),
        ("timed out", "Network problem: the site stopped answering"),
        ("no space left", "The disk is full"),
    ];
    if let Some((_, msg)) = known.iter().find(|(k, _)| l.contains(k)) {
        return (*msg).to_string();
    }
    // "ERROR: [youtube] abc123: Something" -> "Something"
    let mut msg = raw.trim().trim_start_matches("ERROR:").trim();
    if msg.starts_with('[') {
        if let Some(i) = msg.find("]") {
            msg = msg[i + 1..].trim();
            if let Some((head, rest)) = msg.split_once(": ") {
                if !head.contains(' ') {
                    msg = rest;
                }
            }
        }
    }
    let mut out: String = msg.chars().take(160).collect();
    if out.is_empty() {
        out = "yt-dlp stopped with an error".into();
    }
    out
}

/// A Windows job object that kills every process in it when told to, or when dropped.
struct KillGroup(windows_sys::Win32::Foundation::HANDLE);

// The handle is only used through thread-safe Win32 calls.
unsafe impl Send for KillGroup {}
unsafe impl Sync for KillGroup {}

impl KillGroup {
    fn new() -> Option<Self> {
        use windows_sys::Win32::System::JobObjects::*;
        unsafe {
            let h = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if h.is_null() {
                return None;
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                h,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const std::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            Some(Self(h))
        }
    }
    fn add(&self, process: std::os::windows::io::RawHandle) {
        unsafe {
            windows_sys::Win32::System::JobObjects::AssignProcessToJobObject(self.0, process as _);
        }
    }
    fn kill(&self) {
        unsafe {
            windows_sys::Win32::System::JobObjects::TerminateJobObject(self.0, 1);
        }
    }
}

impl Drop for KillGroup {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Value {
        // A trimmed, synthetic `yt-dlp -J` answer shaped like YouTube's (ids as YouTube uses them).
        serde_json::json!({
            "title": "Test clip", "channel": "Some channel", "duration": 600.0,
            "extractor_key": "Youtube", "webpage_url": "https://www.youtube.com/watch?v=x",
            "thumbnails": [
                {"url": "https://i.ytimg.com/vi/x/hqdefault.jpg", "width": 480, "height": 360},
                {"url": "https://i.ytimg.com/vi/x/mqdefault.jpg", "width": 320, "height": 180},
                {"url": "https://i.ytimg.com/vi/x/maxresdefault.jpg", "width": 1280, "height": 720},
                {"url": "https://i.ytimg.com/vi/x/hq720.webp", "width": 686, "height": 386}
            ],
            "formats": [
                {"format_id": "sb0", "ext": "mhtml", "format_note": "storyboard", "vcodec": "none", "acodec": "none"},
                {"format_id": "139", "ext": "m4a", "vcodec": "none", "acodec": "mp4a.40.5", "abr": 48.0, "filesize": 3_600_000},
                {"format_id": "140", "ext": "m4a", "vcodec": "none", "acodec": "mp4a.40.2", "abr": 129.5, "filesize": 9_700_000},
                {"format_id": "140-drc", "ext": "m4a", "vcodec": "none", "acodec": "mp4a.40.2", "abr": 129.5, "filesize": 9_700_000},
                {"format_id": "251", "ext": "webm", "vcodec": "none", "acodec": "opus", "abr": 135.0, "filesize": 10_100_000},
                {"format_id": "18", "ext": "mp4", "vcodec": "avc1.42001E", "acodec": "mp4a.40.2", "width": 640, "height": 360, "fps": 30.0, "tbr": 500.0, "filesize_approx": 37_000_000},
                {"format_id": "136", "ext": "mp4", "vcodec": "avc1.4d401f", "acodec": "none", "width": 1280, "height": 720, "fps": 30.0, "tbr": 1500.0, "filesize": 110_000_000},
                {"format_id": "247", "ext": "webm", "vcodec": "vp9", "acodec": "none", "width": 1280, "height": 720, "fps": 30.0, "tbr": 1400.0, "filesize": 100_000_000},
                {"format_id": "137", "ext": "mp4", "vcodec": "avc1.640028", "acodec": "none", "width": 1920, "height": 1080, "fps": 30.0, "tbr": 4000.0, "filesize": 300_000_000},
                {"format_id": "299", "ext": "mp4", "vcodec": "avc1.64002a", "acodec": "none", "width": 1920, "height": 1080, "fps": 60.0, "tbr": 6000.0, "filesize": 450_000_000},
                {"format_id": "303", "ext": "webm", "vcodec": "vp9", "acodec": "none", "width": 1920, "height": 1080, "fps": 60.0, "tbr": 4400.0, "filesize": 330_000_000},
                {"format_id": "401", "ext": "mp4", "vcodec": "av01.0.12M.08", "acodec": "none", "width": 3840, "height": 2160, "fps": 60.0, "tbr": 18000.0, "filesize_approx": 1_350_000_000}
            ]
        })
    }

    #[test]
    fn groups_formats_into_choices() {
        let v = info_from_json(&fixture());
        assert_eq!(v.title, "Test clip");
        assert_eq!(v.site, "YouTube");
        assert_eq!(v.duration, Some(600));
        assert_eq!(v.thumbnail.as_deref(), Some("https://i.ytimg.com/vi/x/mqdefault.jpg"));
        let labels: Vec<String> = v.options.iter().map(|o| format!("{} {}", o.label, o.ext)).collect();
        assert_eq!(
            labels,
            [
                "2160p mkv", // AV1 + opus: no MP4/H.264 at 4K
                "1080p webm",
                "1080p mp4",
                "720p webm",
                "720p mp4",
                "360p mp4",
                "M4A m4a",
                "MP3 mp3",
                "Opus opus",
                "FLAC flac"
            ]
        );
        let p1080 = v.options.iter().find(|o| o.choice == Choice::Video { height: 1080, compat: true }).unwrap();
        // The 60 fps H.264 stream with the best (non-DRC) AAC audio.
        assert!(p1080.selector.starts_with("299+140/"), "{}", p1080.selector);
        assert_eq!(p1080.size, Some(459_700_000));
        assert!(p1080.recommended);
        assert!(p1080.detail.contains("60 fps"));
        let p360 = v.options.iter().find(|o| o.label == "360p").unwrap();
        assert!(p360.selector.starts_with("18/"), "a single stream with audio needs no merge");
        assert!(p360.approx);
        let flac = v.options.iter().find(|o| o.label == "FLAC").unwrap();
        assert!(flac.note.is_some(), "FLAC from a lossy source must say so");
        assert!(flac.selector.starts_with("251/"));
        let m4a = v.options.iter().find(|o| o.label == "M4A").unwrap();
        assert!(m4a.selector.starts_with("140/"));
        assert!(m4a.recommended);
    }

    #[test]
    fn widescreen_and_vertical_get_their_usual_names() {
        let f = |w, h| Fmt {
            id: String::new(), ext: String::new(), vcodec: None, acodec: None, height: h, width: w, fps: 0.0, hdr: false,
            size: None, approx: false, tbr: 0.0, abr: 0.0, lang_pref: 0,
        };
        assert_eq!(step_of(f(1920, 800).lines()), Some(1080));
        assert_eq!(step_of(f(1080, 1920).lines()), Some(1080));
        assert_eq!(step_of(f(3840, 1608).lines()), Some(2160));
        assert_eq!(step_of(f(854, 480).lines()), Some(480));
    }

    #[test]
    fn parses_progress_lines() {
        assert_eq!(
            parse_line("DASP|137|downloading|1048576|300000000|NA|2500000.5|119|C:\\x\\Clip.f137.mp4.part"),
            Line::Progress {
                format: "137".into(),
                status: "downloading".into(),
                downloaded: 1_048_576,
                total: Some(300_000_000),
                speed: Some(2_500_000.5),
                eta: Some(119),
                tmp: "C:\\x\\Clip.f137.mp4.part".into(),
            }
        );
        // Unknown total: falls back to the estimate.
        let Line::Progress { total, speed, .. } = parse_line("DASP|251|downloading|10|NA|5000.0|NA|NA|a|b.part") else { panic!() };
        assert_eq!((total, speed), (Some(5000), None));
        assert_eq!(parse_line("DASQ|Merger|started"), Line::Post { processor: "Merger".into(), status: "started".into() });
        assert_eq!(parse_line("DASF|C:\\Videos\\Clip.mp4\r"), Line::Final(PathBuf::from("C:\\Videos\\Clip.mp4")));
        assert_eq!(parse_line("[download] something"), Line::Other);
    }

    #[test]
    fn tracks_video_and_audio_streams() {
        let mut st = VideoState::default();
        for l in [
            "DASP|137|downloading|100|1000|NA|50.0|NA|v.part",
            "DASP|137|finished|1000|1000|NA|NA|NA|v.part",
            "DASP|140|downloading|50|200|NA|40.0|NA|a.part",
        ] {
            st.apply(parse_line(l));
        }
        assert_eq!(st.downloaded(), 1050);
        assert_eq!(st.known_total(), 1200);
        assert_eq!(st.phase, VideoPhase::Downloading);
        st.apply(parse_line("DASQ|Merger|started"));
        assert_eq!(st.phase, VideoPhase::Merging);
        assert_eq!(st.partials, ["v.part", "a.part"]);
    }

    #[test]
    fn choice_json_matches_the_ui() {
        let v: Choice = serde_json::from_str(r#"{"kind":"video","height":1080,"compat":true}"#).unwrap();
        assert_eq!(v, Choice::Video { height: 1080, compat: true });
        let a: Choice = serde_json::from_str(r#"{"kind":"audio","format":"mp3"}"#).unwrap();
        assert_eq!(a, Choice::Audio { format: AudioFormat::Mp3 });
        let o: VideoOption = serde_json::from_str(
            r#"{"choice":{"kind":"audio","format":"flac"},"label":"FLAC","detail":"x","ext":"flac","size":null,"approx":true,"selector":"ba/b","note":"n","recommended":false}"#,
        )
        .unwrap();
        assert_eq!(o.ext, "flac");
        assert_eq!(serde_json::to_value(VideoPhase::Merging).unwrap(), "merging");
    }

    #[test]
    fn knows_video_sites() {
        assert!(is_video_site("https://youtu.be/abc?si=1"));
        assert!(is_video_site("https://m.youtube.com/watch?v=x"));
        assert!(is_video_site("https://music.youtube.com/watch?v=x"));
        assert!(!is_video_site("https://notyoutube.com/x"));
        assert!(!is_video_site("https://cdn.example.net/file.zip"));
    }

    #[test]
    fn friendly_errors() {
        assert_eq!(friendly_error("ERROR: [youtube] abc: Private video. Sign in if you've been granted access"), "This video is private");
        assert_eq!(friendly_error("ERROR: [generic] Unsupported URL: https://x"), "This site isn't supported for video");
        assert_eq!(friendly_error("ERROR: [vimeo] 123: Something odd happened"), "Something odd happened");
    }
}
