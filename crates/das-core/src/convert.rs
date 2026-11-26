//! Convert music the user owns into another format or quality with the bundled ffmpeg.
//!
//! One batch (a file, a few files, or a folder) is one list item. Files are written to a
//! temporary `.dasconv` name and renamed only when ffmpeg finishes, so a pause, crash or quit
//! never leaves a half file under a real name. Originals are never touched.

use crate::tools::hidden;
use crate::video::{KillGroup, next_text};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::BufReader;

/// The formats and qualities on offer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Preset {
    #[serde(rename = "mp3-320")]
    Mp3_320,
    #[serde(rename = "mp3-v0")]
    Mp3V0,
    #[serde(rename = "mp3-256")]
    Mp3_256,
    #[serde(rename = "mp3-192")]
    Mp3_192,
    #[serde(rename = "aac-256")]
    Aac256,
    #[serde(rename = "opus-160")]
    Opus160,
    #[serde(rename = "opus-128")]
    Opus128,
    #[serde(rename = "vorbis-q6")]
    VorbisQ6,
    #[serde(rename = "alac")]
    Alac,
    #[serde(rename = "flac")]
    Flac,
    #[serde(rename = "flac-16")]
    Flac16,
    #[serde(rename = "wav-16")]
    Wav16,
}

pub const PRESETS: [Preset; 12] = [
    Preset::Mp3_320,
    Preset::Mp3V0,
    Preset::Mp3_256,
    Preset::Mp3_192,
    Preset::Aac256,
    Preset::Opus160,
    Preset::Opus128,
    Preset::VorbisQ6,
    Preset::Alac,
    Preset::Flac,
    Preset::Flac16,
    Preset::Wav16,
];

impl Preset {
    /// Also the name of the folder the files go to.
    pub fn label(self) -> &'static str {
        match self {
            Preset::Mp3_320 => "MP3 320",
            Preset::Mp3V0 => "MP3 V0",
            Preset::Mp3_256 => "MP3 256",
            Preset::Mp3_192 => "MP3 192",
            Preset::Aac256 => "AAC 256",
            Preset::Opus160 => "Opus 160",
            Preset::Opus128 => "Opus 128",
            Preset::VorbisQ6 => "Ogg Vorbis",
            Preset::Alac => "ALAC",
            Preset::Flac => "FLAC",
            Preset::Flac16 => "FLAC 16-bit",
            Preset::Wav16 => "WAV 16-bit",
        }
    }
    pub fn ext(self) -> &'static str {
        match self {
            Preset::Mp3_320 | Preset::Mp3V0 | Preset::Mp3_256 | Preset::Mp3_192 => "mp3",
            Preset::Aac256 | Preset::Alac => "m4a",
            Preset::Opus160 | Preset::Opus128 => "opus",
            Preset::VorbisQ6 => "ogg",
            Preset::Flac | Preset::Flac16 => "flac",
            Preset::Wav16 => "wav",
        }
    }
    /// The codec ffprobe reports for the result.
    pub fn codec(self) -> &'static str {
        match self {
            Preset::Mp3_320 | Preset::Mp3V0 | Preset::Mp3_256 | Preset::Mp3_192 => "mp3",
            Preset::Aac256 => "aac",
            Preset::Opus160 | Preset::Opus128 => "opus",
            Preset::VorbisQ6 => "vorbis",
            Preset::Alac => "alac",
            Preset::Flac | Preset::Flac16 => "flac",
            Preset::Wav16 => "pcm_s16le",
        }
    }
    pub fn lossless(self) -> bool {
        matches!(self, Preset::Alac | Preset::Flac | Preset::Flac16 | Preset::Wav16)
    }
    /// Roughly the bitrate of a lossy result, bits per second (None = lossless or variable).
    pub fn bitrate(self) -> Option<u64> {
        match self {
            Preset::Mp3_320 => Some(320_000),
            Preset::Mp3V0 => Some(245_000),
            Preset::Mp3_256 | Preset::Aac256 => Some(256_000),
            Preset::Mp3_192 | Preset::VorbisQ6 => Some(192_000),
            Preset::Opus160 => Some(160_000),
            Preset::Opus128 => Some(128_000),
            _ => None,
        }
    }
    /// Whether the cover picture is carried over (Ogg and Opus: see `keeps_cover` in the tests).
    pub fn keeps_cover(self) -> bool {
        !matches!(self, Preset::Opus160 | Preset::Opus128 | Preset::VorbisQ6 | Preset::Wav16)
    }
    fn muxer(self) -> &'static str {
        match self.ext() {
            "mp3" => "mp3",
            "m4a" => "ipod",
            "opus" => "opus",
            "ogg" => "ogg",
            "flac" => "flac",
            _ => "wav",
        }
    }
}

/// What a file is, from ffprobe.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioProbe {
    pub codec: String,
    pub sample_rate: u32,
    pub channels: u32,
    /// Bit depth for lossless audio (16, 24).
    pub bits: Option<u32>,
    /// Bits per second (lossy files, and the average for lossless ones).
    pub bitrate: Option<u64>,
    /// Seconds.
    pub duration: f64,
    pub cover: bool,
    pub lossless: bool,
}

impl AudioProbe {
    /// "FLAC 24-bit 96 kHz", "MP3 320 kbps".
    pub fn describe(&self) -> String {
        let name = match self.codec.as_str() {
            c if c.starts_with("pcm_") => "WAV".to_string(),
            "vorbis" => "Ogg Vorbis".into(),
            c => c.to_uppercase(),
        };
        let khz = |r: u32| if r % 1000 == 0 { format!("{} kHz", r / 1000) } else { format!("{:.1} kHz", r as f64 / 1000.0) };
        if self.lossless {
            format!("{name} {}{}", self.bits.map(|b| format!("{b}-bit ")).unwrap_or_default(), khz(self.sample_rate))
        } else {
            format!("{name} {}", self.bitrate.map(|b| format!("{} kbps", (b + 500) / 1000)).unwrap_or_else(|| khz(self.sample_rate)))
        }
    }
}

fn is_lossless_codec(c: &str) -> bool {
    matches!(c, "flac" | "alac" | "wavpack" | "ape" | "tta" | "truehd" | "mlp" | "shorten") || c.starts_with("pcm_")
}

/// Read ffprobe's JSON (`-show_streams -show_format`).
pub fn parse_probe(json: &[u8]) -> Option<AudioProbe> {
    let v: serde_json::Value = serde_json::from_slice(json).ok()?;
    let streams = v["streams"].as_array()?;
    let audio = streams.iter().find(|s| s["codec_type"] == "audio")?;
    let num = |x: &serde_json::Value| x.as_str().and_then(|s| s.parse::<u64>().ok()).or(x.as_u64());
    let codec = audio["codec_name"].as_str()?.to_string();
    let lossless = is_lossless_codec(&codec);
    let bits = num(&audio["bits_per_raw_sample"]).or(num(&audio["bits_per_sample"])).filter(|b| *b > 0).map(|b| b as u32);
    let duration = audio["duration"].as_str().or(v["format"]["duration"].as_str()).and_then(|d| d.parse().ok()).unwrap_or(0.0);
    Some(AudioProbe {
        sample_rate: num(&audio["sample_rate"]).unwrap_or(0) as u32,
        channels: num(&audio["channels"]).unwrap_or(0) as u32,
        bits: if lossless { bits } else { None },
        bitrate: num(&audio["bit_rate"]).or(num(&v["format"]["bit_rate"])),
        duration,
        cover: streams.iter().any(|s| s["codec_type"] == "video" && s["disposition"]["attached_pic"] == 1),
        lossless,
        codec,
    })
}

/// The sample rate a 16-bit result gets: 88.2 / 176.4 kHz go to 44.1, 96 / 192 kHz to 48;
/// 48 kHz and below stay as they are.
pub fn target_rate(src: u32) -> u32 {
    if src <= 48_000 {
        src
    } else if src % 11_025 == 0 {
        44_100
    } else {
        48_000
    }
}

/// Whether a conversion makes sense, and what to tell the user.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "note")]
pub enum Verdict {
    Ok,
    /// Fine but worth knowing.
    Warn(String),
    /// Nothing to gain: the file is left out.
    Skip(String),
}

pub fn verdict(p: &AudioProbe, preset: Preset) -> Verdict {
    let same = p.codec == preset.codec() || (p.codec.starts_with("pcm_") && preset == Preset::Wav16);
    if p.lossless {
        let deep = p.bits.unwrap_or(16) > 16 || p.sample_rate > 48_000;
        return match preset {
            Preset::Flac if same => Verdict::Skip("Already FLAC".into()),
            Preset::Flac16 | Preset::Wav16 if same && !deep => Verdict::Skip("Already 16-bit".into()),
            _ => Verdict::Ok,
        };
    }
    if preset.lossless() {
        return Verdict::Warn("From a lossy file this won't sound better, only bigger".into());
    }
    let src = p.bitrate.unwrap_or(0);
    // Same codec at the same or a higher bitrate: a re-encode only loses quality.
    if same && preset.bitrate().is_some_and(|b| b >= src * 95 / 100) {
        return Verdict::Skip(format!("Already {} at this quality or lower", preset.codec().to_uppercase()));
    }
    Verdict::Warn("Lossy to lossy loses a little more quality".into())
}

/// ffmpeg's arguments for one file. Progress goes to stdout as `key=value` lines.
pub fn args(preset: Preset, probe: &AudioProbe, input: &Path, output: &Path) -> Vec<String> {
    let mut a: Vec<String> = ["-hide_banner", "-nostdin", "-y", "-i"].map(String::from).to_vec();
    a.push(input.to_string_lossy().into_owned());
    a.extend(["-map", "0:a:0", "-map_metadata", "0"].map(String::from));
    if probe.cover && preset.keeps_cover() {
        a.extend(["-map", "0:v:0?", "-c:v", "copy", "-disposition:v:0", "attached_pic"].map(String::from));
    }
    let codec: &[&str] = match preset {
        Preset::Mp3_320 => &["-c:a", "libmp3lame", "-b:a", "320k"],
        Preset::Mp3V0 => &["-c:a", "libmp3lame", "-q:a", "0"],
        Preset::Mp3_256 => &["-c:a", "libmp3lame", "-b:a", "256k"],
        Preset::Mp3_192 => &["-c:a", "libmp3lame", "-b:a", "192k"],
        Preset::Aac256 => &["-c:a", "aac", "-b:a", "256k"],
        Preset::Opus160 => &["-c:a", "libopus", "-b:a", "160k"],
        Preset::Opus128 => &["-c:a", "libopus", "-b:a", "128k"],
        Preset::VorbisQ6 => &["-c:a", "libvorbis", "-q:a", "6"],
        Preset::Alac => &["-c:a", "alac"],
        Preset::Flac => &["-c:a", "flac", "-compression_level", "8"],
        Preset::Flac16 => &["-c:a", "flac", "-compression_level", "8", "-sample_fmt", "s16"],
        Preset::Wav16 => &["-c:a", "pcm_s16le"],
    };
    a.extend(codec.iter().map(|s| s.to_string()));
    if matches!(preset, Preset::Flac16 | Preset::Wav16) {
        // High-quality resampling, and dither when cutting the bit depth.
        a.push("-af".into());
        a.push(format!(
            "aresample=osr={}:osf=s16:resampler=soxr:precision=28:dither_method=triangular",
            target_rate(probe.sample_rate.max(1))
        ));
    } else if !preset.lossless() && probe.sample_rate > 48_000 {
        // Lossy files above 48 kHz only waste bits, and many phones and car stereos won't play them.
        a.push("-af".into());
        a.push(format!("aresample=osr={}:resampler=soxr:precision=28", target_rate(probe.sample_rate)));
    }
    if preset.ext() == "mp3" {
        a.extend(["-id3v2_version", "3", "-write_id3v1", "0"].map(String::from));
    }
    a.extend(["-progress", "pipe:1", "-nostats", "-f", preset.muxer()].map(String::from));
    a.push(output.to_string_lossy().into_owned());
    a
}

/// Audio files DasIDM can convert from.
pub fn is_audio_file(p: &Path) -> bool {
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    matches!(ext.as_str(), "flac" | "wav" | "aif" | "aiff" | "alac" | "m4a" | "mp3" | "ogg" | "opus" | "wv" | "ape" | "aac" | "wma")
}

/// The files of a batch: files as given, folders searched (skipping folders DasIDM itself
/// writes conversions to). Each comes with the folder it was found under, so subfolders are kept.
pub fn expand(inputs: &[PathBuf]) -> Vec<(PathBuf, Option<PathBuf>)> {
    const MAX_FILES: usize = 5000;
    const MAX_DEPTH: usize = 12;
    let ours: Vec<&str> = PRESETS.iter().map(|p| p.label()).collect();
    let mut out = Vec::new();
    for i in inputs {
        if i.is_dir() {
            let mut stack = vec![(i.clone(), 0usize)];
            while let Some((d, depth)) = stack.pop() {
                if out.len() >= MAX_FILES {
                    break;
                }
                let Ok(rd) = std::fs::read_dir(&d) else { continue };
                let mut entries: Vec<(PathBuf, std::fs::FileType)> = rd.filter_map(|e| e.ok()).filter_map(|e| Some((e.path(), e.file_type().ok()?))).collect();
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                for (p, ft) in entries {
                    // Never follow links or junctions (a dropped profile folder must not loop).
                    if ft.is_symlink() || is_reparse_point(&p) {
                        continue;
                    }
                    if ft.is_dir() {
                        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        if !ours.contains(&name) && depth < MAX_DEPTH {
                            stack.push((p, depth + 1));
                        }
                    } else if is_audio_file(&p) && out.len() < MAX_FILES {
                        out.push((p, Some(i.clone())));
                    }
                }
            }
        } else if is_audio_file(i) {
            out.push((i.clone(), None));
        }
    }
    out
}

fn is_reparse_point(p: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        std::fs::symlink_metadata(p).is_ok_and(|m| m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0)
    }
    #[cfg(not(windows))]
    {
        let _ = p;
        false
    }
}

/// Where one file's result goes: next to the original in a folder named after the preset
/// (`Album\MP3 320\`), or under a chosen folder; subfolders of a folder input are kept.
pub fn output_dir(input: &Path, root: Option<&Path>, chosen: Option<&Path>, preset: Preset) -> PathBuf {
    let parent = input.parent().unwrap_or(Path::new("."));
    let rel = root.and_then(|r| parent.strip_prefix(r).ok()).map(Path::to_path_buf).unwrap_or_default();
    match (chosen, root) {
        (Some(c), _) => c.join(rel),
        (None, Some(r)) => r.join(preset.label()).join(rel),
        (None, None) => parent.join(preset.label()),
    }
}

/// One file of a batch, as stored.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertFile {
    pub input: String,
    #[serde(default)]
    pub root: Option<String>,
    #[serde(default)]
    pub size: u64,
    /// Seconds (from ffprobe).
    #[serde(default)]
    pub duration: f64,
    pub state: FileState,
    /// The result DasIDM made (deleted with "delete files"; nothing else ever is).
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileState {
    Pending,
    Done,
    Skipped,
    Failed,
}

/// What a convert item remembers.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertSpec {
    pub preset: Preset,
    /// A folder chosen for all results (None = next to the originals).
    #[serde(default)]
    pub out_dir: Option<String>,
    pub files: Vec<ConvertFile>,
}

impl ConvertSpec {
    /// Seconds of audio in the batch and seconds already done (skipped files count as done).
    pub fn seconds(&self) -> (f64, f64) {
        let total: f64 = self.files.iter().map(|f| f.duration).sum();
        let done: f64 = self.files.iter().filter(|f| f.state != FileState::Pending).map(|f| f.duration).sum();
        (total, done)
    }
}

/// A file finished, was skipped or failed.
pub struct FileOutcome {
    pub index: usize,
    pub state: FileState,
    pub output: Option<PathBuf>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvertEnd {
    Done,
    Paused,
    Cancelled,
}

/// A running batch.
#[derive(Clone)]
pub struct ConvertJob {
    s: Arc<JobShared>,
}

struct JobShared {
    stop: AtomicBool,
    cancel: AtomicBool,
    groups: Mutex<Vec<Arc<KillGroup>>>,
    /// Seconds converted so far in the files running now.
    live: Mutex<std::collections::HashMap<usize, f64>>,
    done: tokio::sync::watch::Sender<Option<ConvertEnd>>,
}

impl ConvertJob {
    /// Convert every pending file of `spec`; `on_file` hears about each file as it ends.
    pub fn start(
        ffmpeg: PathBuf,
        ffprobe: PathBuf,
        spec: ConvertSpec,
        parallel: usize,
        on_file: Arc<dyn Fn(FileOutcome) + Send + Sync>,
    ) -> Self {
        let (tx, _) = tokio::sync::watch::channel(None);
        let s = Arc::new(JobShared {
            stop: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
            groups: Mutex::new(Vec::new()),
            live: Mutex::new(Default::default()),
            done: tx,
        });
        let job = Self { s: s.clone() };
        tokio::spawn(async move {
            let sem = Arc::new(tokio::sync::Semaphore::new(parallel.max(1)));
            let mut set = tokio::task::JoinSet::new();
            for (index, f) in spec.files.iter().enumerate() {
                if f.state != FileState::Pending {
                    continue;
                }
                let permit = sem.clone().acquire_owned().await.unwrap();
                if s.stop.load(Ordering::SeqCst) {
                    break;
                }
                let (s2, on_file, ffmpeg, ffprobe, f) = (s.clone(), on_file.clone(), ffmpeg.clone(), ffprobe.clone(), f.clone());
                let (preset, out_dir) = (spec.preset, spec.out_dir.clone());
                set.spawn(async move {
                    let outcome = convert_one(&s2, &ffmpeg, &ffprobe, preset, out_dir.as_deref(), index, &f).await;
                    s2.live.lock().remove(&index);
                    if let Some(o) = outcome {
                        on_file(o);
                    }
                    drop(permit);
                });
            }
            while set.join_next().await.is_some() {}
            let end = if s.cancel.load(Ordering::SeqCst) {
                ConvertEnd::Cancelled
            } else if s.stop.load(Ordering::SeqCst) {
                ConvertEnd::Paused
            } else {
                ConvertEnd::Done
            };
            let _ = s.done.send(Some(end));
        });
        job
    }

    pub fn pause(&self) {
        self.s.stop.store(true, Ordering::SeqCst);
        for g in self.s.groups.lock().iter() {
            g.kill();
        }
    }

    pub fn cancel(&self) {
        self.s.cancel.store(true, Ordering::SeqCst);
        self.pause();
    }

    /// Seconds converted so far in the files that are running now.
    pub fn live_seconds(&self) -> f64 {
        self.s.live.lock().values().sum()
    }

    pub async fn wait(&self) -> ConvertEnd {
        let mut rx = self.s.done.subscribe();
        loop {
            if let Some(e) = rx.borrow_and_update().clone() {
                return e;
            }
            if rx.changed().await.is_err() {
                return ConvertEnd::Cancelled;
            }
        }
    }
}

/// Files already read, by path, size and modified time: the sheet asks again for every format
/// change, then the batch and each file's conversion ask once more.
static PROBES: std::sync::LazyLock<Mutex<std::collections::HashMap<(PathBuf, u64, Option<std::time::SystemTime>), AudioProbe>>> =
    std::sync::LazyLock::new(Default::default);

/// ffprobe one file (cached).
pub async fn probe(ffprobe: &Path, file: &Path) -> Option<AudioProbe> {
    let meta = std::fs::metadata(file).ok()?;
    let key = (file.to_path_buf(), meta.len(), meta.modified().ok());
    if let Some(p) = PROBES.lock().get(&key) {
        return Some(p.clone());
    }
    let p = probe_uncached(ffprobe, file).await?;
    let mut cache = PROBES.lock();
    if cache.len() > 20_000 {
        cache.clear();
    }
    cache.insert(key, p.clone());
    Some(p)
}

async fn probe_uncached(ffprobe: &Path, file: &Path) -> Option<AudioProbe> {
    let out = hidden(ffprobe)
        .args(["-v", "error", "-print_format", "json", "-show_streams", "-show_format"])
        .arg(file)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .ok()?;
    parse_probe(&out.stdout)
}

/// None when stopped (the file stays pending).
async fn convert_one(
    s: &JobShared,
    ffmpeg: &Path,
    ffprobe: &Path,
    preset: Preset,
    out_dir: Option<&str>,
    index: usize,
    f: &ConvertFile,
) -> Option<FileOutcome> {
    let input = PathBuf::from(&f.input);
    let fail = |note: String| Some(FileOutcome { index, state: FileState::Failed, output: None, note: Some(note) });
    let Some(p) = probe(ffprobe, &input).await else {
        return fail("Not an audio file ffmpeg can read".into());
    };
    if let Verdict::Skip(note) = verdict(&p, preset) {
        return Some(FileOutcome { index, state: FileState::Skipped, output: None, note: Some(note) });
    }
    let dir = output_dir(&input, f.root.as_deref().map(Path::new), out_dir.map(Path::new), preset);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return fail(format!("Can't create {}: {e}", dir.display()));
    }
    let stem = input.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "track".into());
    let name = format!("{stem}.{}", preset.ext());
    let tmp = dir.join(format!("{name}.dasconv"));
    let mut cmd = hidden(ffmpeg);
    cmd.args(args(preset, &p, &input, &tmp)).stdout(Stdio::piped()).stderr(Stdio::piped());
    let Ok(mut child) = cmd.spawn() else { return fail("Could not run ffmpeg".into()) };
    let group = KillGroup::new().map(Arc::new);
    if let (Some(g), Some(h)) = (&group, child.raw_handle()) {
        g.add(h);
        s.groups.lock().push(g.clone());
    }
    if s.stop.load(Ordering::SeqCst) {
        let _ = child.start_kill();
    }
    let stderr = child.stderr.take().map(|e| {
        tokio::spawn(async move {
            let (mut r, mut buf, mut last) = (BufReader::new(e), Vec::new(), String::new());
            while let Some(l) = next_text(&mut r, &mut buf).await {
                if !l.trim().is_empty() {
                    last = l;
                }
            }
            last
        })
    });
    if let Some(out) = child.stdout.take() {
        let (mut r, mut buf) = (BufReader::new(out), Vec::new());
        while let Some(l) = next_text(&mut r, &mut buf).await {
            if let Some(us) = l.strip_prefix("out_time_us=").and_then(|v| v.trim().parse::<i64>().ok()) {
                s.live.lock().insert(index, (us.max(0) as f64 / 1e6).min(p.duration.max(0.0)));
            }
        }
    }
    let status = child.wait().await;
    let last_err = match stderr {
        Some(t) => t.await.unwrap_or_default(),
        None => String::new(),
    };
    if let Some(g) = &group {
        s.groups.lock().retain(|x| !Arc::ptr_eq(x, g));
    }
    if s.stop.load(Ordering::SeqCst) {
        let _ = std::fs::remove_file(&tmp);
        return None;
    }
    if !status.is_ok_and(|st| st.success()) {
        let _ = std::fs::remove_file(&tmp);
        return fail(if last_err.is_empty() { "ffmpeg stopped with an error".into() } else { last_err.chars().take(160).collect() });
    }
    let final_path = das_engine::filename::unique_path(&dir, &name);
    if let Err(e) = std::fs::rename(&tmp, &final_path) {
        let _ = std::fs::remove_file(&tmp);
        return fail(format!("Couldn't save the result: {e}"));
    }
    Some(FileOutcome { index, state: FileState::Done, output: Some(final_path), note: None })
}

/// Leftover `.dasconv` files from a batch that was stopped without a chance to clean up.
pub fn remove_temps(spec: &ConvertSpec) {
    for f in &spec.files {
        if f.state != FileState::Pending {
            continue;
        }
        let input = Path::new(&f.input);
        let dir = output_dir(input, f.root.as_deref().map(Path::new), spec.out_dir.as_deref().map(Path::new), spec.preset);
        let stem = input.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let _ = std::fs::remove_file(dir.join(format!("{stem}.{}.dasconv", spec.preset.ext())));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe_of(codec: &str, rate: u32, bits: Option<u32>, bitrate: Option<u64>) -> AudioProbe {
        AudioProbe { codec: codec.into(), sample_rate: rate, channels: 2, bits, bitrate, duration: 10.0, cover: true, lossless: is_lossless_codec(codec) }
    }

    #[test]
    fn reads_ffprobe_json() {
        let json = br#"{"streams":[{"index":0,"codec_name":"flac","codec_type":"audio","sample_rate":"96000","channels":2,"bits_per_raw_sample":"24","duration":"183.4"},
            {"index":1,"codec_name":"mjpeg","codec_type":"video","disposition":{"attached_pic":1}}],"format":{"duration":"183.400000","bit_rate":"3501000"}}"#;
        let p = parse_probe(json).unwrap();
        assert_eq!((p.codec.as_str(), p.sample_rate, p.bits, p.cover, p.lossless), ("flac", 96_000, Some(24), true, true));
        assert!((p.duration - 183.4).abs() < 1e-6);
        assert_eq!(p.describe(), "FLAC 24-bit 96 kHz");
        let mp3 = parse_probe(br#"{"streams":[{"codec_name":"mp3","codec_type":"audio","sample_rate":"44100","channels":2,"bit_rate":"320000"}],"format":{"duration":"60"}}"#).unwrap();
        assert_eq!((mp3.lossless, mp3.bits, mp3.cover), (false, None, false));
        assert_eq!(mp3.describe(), "MP3 320 kbps");
        assert!(parse_probe(b"{}").is_none());
        assert!(parse_probe(br#"{"streams":[{"codec_type":"video","codec_name":"h264"}]}"#).is_none());
    }

    #[test]
    fn sixteen_bit_rates() {
        assert_eq!([44_100, 48_000, 32_000, 88_200, 176_400, 96_000, 192_000, 352_800].map(target_rate), [44_100, 48_000, 32_000, 44_100, 44_100, 48_000, 48_000, 44_100]);
    }

    #[test]
    fn honest_verdicts() {
        let hires = probe_of("flac", 96_000, Some(24), None);
        let cd = probe_of("flac", 44_100, Some(16), None);
        let mp3 = probe_of("mp3", 44_100, None, Some(320_000));
        let mp3_128 = probe_of("mp3", 44_100, None, Some(128_000));
        assert_eq!(verdict(&hires, Preset::Mp3_320), Verdict::Ok);
        assert_eq!(verdict(&hires, Preset::Flac16), Verdict::Ok);
        assert!(matches!(verdict(&hires, Preset::Flac), Verdict::Skip(_)));
        assert!(matches!(verdict(&cd, Preset::Flac16), Verdict::Skip(_)));
        assert_eq!(verdict(&cd, Preset::Alac), Verdict::Ok);
        assert!(matches!(verdict(&mp3, Preset::Flac), Verdict::Warn(n) if n.contains("only bigger")));
        assert!(matches!(verdict(&mp3, Preset::Aac256), Verdict::Warn(n) if n.contains("Lossy to lossy")));
        assert!(matches!(verdict(&mp3, Preset::Mp3_320), Verdict::Skip(_)));
        assert!(matches!(verdict(&mp3_128, Preset::Mp3_192), Verdict::Skip(_)), "128 kbps to 192 gains nothing");
        assert!(matches!(verdict(&mp3, Preset::Mp3_192), Verdict::Warn(_)));
    }

    #[test]
    fn ffmpeg_arguments() {
        let hires = probe_of("flac", 96_000, Some(24), None);
        let a = args(Preset::Flac16, &hires, Path::new("C:\\in\\a.flac"), Path::new("C:\\out\\a.flac.dasconv"));
        let s = a.join(" ");
        assert!(s.contains("-c:a flac") && s.contains("-sample_fmt s16"), "{s}");
        assert!(s.contains("aresample=osr=48000:osf=s16:resampler=soxr"), "{s}");
        assert!(s.contains("-map 0:v:0? -c:v copy -disposition:v:0 attached_pic"), "{s}");
        assert!(s.ends_with("-f flac C:\\out\\a.flac.dasconv"), "{s}");
        let m = args(Preset::Mp3_320, &hires, Path::new("a.flac"), Path::new("a.mp3.dasconv")).join(" ");
        assert!(m.contains("-c:a libmp3lame -b:a 320k") && m.contains("-id3v2_version 3") && m.contains("-progress pipe:1"), "{m}");
        let o = args(Preset::Opus128, &hires, Path::new("a.flac"), Path::new("a.opus.dasconv")).join(" ");
        assert!(!o.contains("attached_pic") && o.contains("-f opus"), "{o}");
        assert!(args(Preset::Alac, &hires, Path::new("a"), Path::new("b")).join(" ").contains("-f ipod"));
        let aac = args(Preset::Aac256, &hires, Path::new("a.flac"), Path::new("a.m4a.dasconv")).join(" ");
        assert!(aac.contains("aresample=osr=48000:resampler=soxr") && !aac.contains("osf=s16"), "{aac}");
        let cd = probe_of("flac", 44_100, Some(16), None);
        assert!(!args(Preset::Aac256, &cd, Path::new("a"), Path::new("b")).join(" ").contains("aresample"), "44.1 kHz stays as it is");
    }

    #[test]
    fn where_results_go() {
        let p = Preset::Mp3_320;
        assert_eq!(output_dir(Path::new("D:\\Music\\Album\\01.flac"), None, None, p), Path::new("D:\\Music\\Album\\MP3 320"));
        assert_eq!(
            output_dir(Path::new("D:\\Music\\Album\\CD2\\01.flac"), Some(Path::new("D:\\Music\\Album")), None, p),
            Path::new("D:\\Music\\Album\\MP3 320\\CD2")
        );
        assert_eq!(output_dir(Path::new("D:\\Music\\Album\\01.flac"), None, Some(Path::new("E:\\Phone")), p), Path::new("E:\\Phone"));
        assert!(is_audio_file(Path::new("x.FLAC")) && !is_audio_file(Path::new("cover.jpg")));
    }
}
