//! The audio converter through the Manager, with the real ffmpeg. Sources are sine tones made
//! here; names carry an em dash, a full-width bar, accents and CJK to catch encoding trouble.
//! Skips (passes) when the media tools aren't installed.

use das_core::convert::{PRESETS, Preset};
use das_core::{AddConvertRequest, Manager, Settings, Status};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn tools() -> Option<PathBuf> {
    let dir = std::env::var_os("DASIDM_TOOLS_DIR").map(PathBuf::from).or_else(|| dirs::data_local_dir().map(|d| d.join("DasIDM").join("tools")))?;
    dir.join("ffmpeg").join("ffmpeg.exe").is_file().then(|| dir.join("ffmpeg"))
}

/// A 24-bit / 96 kHz FLAC tone with title, artist and a cover picture.
fn make_source(ff: &Path, dir: &Path, name: &str, seconds: u32, freq: u32) -> PathBuf {
    let cover = dir.join("cover-src.png");
    if !cover.exists() {
        let ok = Command::new(ff.join("ffmpeg.exe"))
            .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "color=c=0x1d3b53:s=200x200:d=1", "-frames:v", "1"])
            .arg(&cover)
            .status()
            .unwrap();
        assert!(ok.success());
    }
    let out = dir.join(name);
    let ok = Command::new(ff.join("ffmpeg.exe"))
        .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i"])
        .arg(format!("sine=frequency={freq}:sample_rate=96000:duration={seconds}"))
        .arg("-i")
        .arg(&cover)
        .args(["-map", "0:a", "-map", "1:v", "-c:a", "flac", "-sample_fmt", "s32", "-bits_per_raw_sample", "24", "-c:v", "png", "-disposition:v", "attached_pic"])
        .args(["-metadata", "title=Tone — Night ｜ 夜", "-metadata", "artist=Café Test"])
        .arg(&out)
        .status()
        .unwrap();
    assert!(ok.success());
    out
}

#[derive(Debug)]
struct Out {
    codec: String,
    rate: u32,
    bits: Option<u32>,
    bitrate: Option<u64>,
    cover: bool,
    title: Option<String>,
}

fn probe(ff: &Path, file: &Path) -> Out {
    let out = Command::new(ff.join("ffprobe.exe"))
        .args(["-v", "error", "-print_format", "json", "-show_streams", "-show_format"])
        .arg(file)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let streams = v["streams"].as_array().unwrap();
    let a = streams.iter().find(|s| s["codec_type"] == "audio").unwrap();
    let num = |x: &serde_json::Value| x.as_str().and_then(|s| s.parse::<u64>().ok()).or(x.as_u64());
    // Tags live on the format (most containers) or on the audio stream (Ogg/Opus).
    let tag = |t: &serde_json::Value| t.as_object().and_then(|o| o.iter().find(|(k, _)| k.eq_ignore_ascii_case("title")).map(|(_, v)| v.as_str().unwrap_or("").to_string()));
    Out {
        codec: a["codec_name"].as_str().unwrap().to_string(),
        rate: num(&a["sample_rate"]).unwrap_or(0) as u32,
        bits: num(&a["bits_per_raw_sample"]).or(num(&a["bits_per_sample"])).map(|b| b as u32).filter(|b| *b > 0),
        bitrate: num(&a["bit_rate"]).or(num(&v["format"]["bit_rate"])),
        cover: streams.iter().any(|s| s["codec_type"] == "video" && s["disposition"]["attached_pic"] == 1),
        title: tag(&v["format"]["tags"]).or_else(|| tag(&a["tags"])),
    }
}

async fn wait_status(m: &Manager, id: &str, want: Status, secs: u64) {
    let start = Instant::now();
    loop {
        let it = m.get(id).unwrap();
        if it.status == want {
            return;
        }
        assert!(it.status != Status::Failed || want == Status::Failed, "failed: {:?}", it.error);
        assert!(start.elapsed() < Duration::from_secs(secs), "timed out waiting for {want:?}, at {:?}", it.status);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn temps(dir: &Path) -> Vec<PathBuf> {
    let mut found = vec![];
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.to_string_lossy().ends_with(".dasconv") {
                found.push(p);
            }
        }
    }
    found
}

fn open(data: &Path) -> Manager {
    let m = Manager::open(data, Arc::new(|_| {})).unwrap();
    m.set_settings(Settings { download_dir: data.join("dl").to_string_lossy().into_owned(), ..m.settings() }).unwrap();
    tokio::spawn(m.clone().run());
    m
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn every_preset_gives_what_it_says() {
    let Some(ff) = tools() else {
        eprintln!("ffmpeg not installed; skipping");
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    let album = tmp.path().join("Álbum — Test");
    std::fs::create_dir_all(&album).unwrap();
    let src = make_source(&ff, &album, "01 Tone — Night ｜ 夜.flac", 4, 440);
    let original = std::fs::read(&src).unwrap();
    let m = open(&tmp.path().join("data"));

    // The sheet's plan: format read, honest verdicts.
    let plan = m.convert_plan(vec![src.to_string_lossy().into_owned()], Preset::Flac, None).await;
    assert!(plan.ffmpeg_ready);
    assert_eq!(plan.files[0].format.as_deref(), Some("FLAC 24-bit 96 kHz"));
    assert!(matches!(plan.files[0].verdict, Some(das_core::convert::Verdict::Skip(_))), "FLAC to the same FLAC is skipped");

    let mut ids = vec![];
    for p in PRESETS.into_iter().filter(|p| *p != Preset::Flac) {
        let v = m.add_convert(AddConvertRequest { paths: vec![src.to_string_lossy().into_owned()], preset: p, out_dir: None }).await.unwrap();
        ids.push((p, v.id));
    }
    for (p, id) in &ids {
        wait_status(&m, id, Status::Done, 120).await;
        let it = m.get(id).unwrap();
        let path = PathBuf::from(it.file_path.clone().unwrap());
        assert_eq!(path.parent().unwrap(), album.join(p.label()), "{p:?} goes next to the original, in a '{}' folder", p.label());
        assert_eq!(path.file_name().unwrap().to_string_lossy(), format!("01 Tone — Night ｜ 夜.{}", p.ext()));
        let o = probe(&ff, &path);
        assert_eq!(o.codec, p.codec(), "{p:?}: {o:?}");
        assert_eq!(o.title.as_deref(), Some("Tone — Night ｜ 夜"), "{p:?} keeps the tags: {o:?}");
        assert_eq!(o.cover, p.keeps_cover(), "{p:?} cover: {o:?}");
        match p {
            Preset::Flac16 | Preset::Wav16 => assert_eq!((o.rate, o.bits), (48_000, Some(16)), "{p:?}: {o:?}"),
            Preset::Alac => assert_eq!((o.rate, o.bits), (96_000, Some(24)), "{p:?}: {o:?}"),
            Preset::Mp3_320 => assert!(o.bitrate.is_some_and(|b| (310_000..=330_000).contains(&b)), "{p:?}: {o:?}"),
            Preset::Mp3_192 => assert!(o.bitrate.is_some_and(|b| (185_000..=200_000).contains(&b)), "{p:?}: {o:?}"),
            Preset::Opus128 | Preset::Opus160 => assert_eq!(o.rate, 48_000, "{p:?}: {o:?}"),
            _ => {}
        }
    }
    assert!(temps(tmp.path()).is_empty());
    assert!(std::fs::read(&src).unwrap() == original, "the original must not change");

    // Remove with "delete files": only the results go.
    let all: Vec<String> = ids.iter().map(|(_, id)| id.clone()).collect();
    let made: Vec<PathBuf> = all.iter().map(|id| PathBuf::from(m.get(id).unwrap().file_path.unwrap())).collect();
    m.remove(&all, true);
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(made.iter().all(|p| !p.exists()), "results deleted");
    assert!(std::fs::read(&src).unwrap() == original, "the original survives delete-files");
    m.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_batch_carries_on_after_quitting() {
    let Some(ff) = tools() else {
        eprintln!("ffmpeg not installed; skipping");
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    let album = tmp.path().join("Live 夜");
    std::fs::create_dir_all(album.join("CD2")).unwrap();
    // Long enough that the batch is still running when DasIDM quits.
    for (i, f) in [220, 330, 440, 550, 660, 770].into_iter().enumerate() {
        let sub = if i < 3 { album.clone() } else { album.join("CD2") };
        make_source(&ff, &sub, &format!("{:02} Café {f}.flac", i + 1), 600, f);
    }
    let data = tmp.path().join("data");
    let m = open(&data);
    // A whole folder: subfolders are kept under "Opus 160".
    let id = m
        .add_convert(AddConvertRequest { paths: vec![album.to_string_lossy().into_owned()], preset: Preset::Opus160, out_dir: None })
        .await
        .unwrap()
        .id;
    wait_status(&m, &id, Status::Downloading, 30).await;
    let start = Instant::now();
    while m.get(&id).unwrap().convert.unwrap().done == 0 {
        assert!(start.elapsed() < Duration::from_secs(120), "no file finished");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Pause stops ffmpeg and leaves no half files; resume finishes.
    m.pause(&[id.clone()]);
    wait_status(&m, &id, Status::Paused, 20).await;
    assert!(temps(tmp.path()).is_empty(), "pause left {:?}", temps(tmp.path()));
    m.resume(&[id.clone()]);
    wait_status(&m, &id, Status::Downloading, 20).await;

    // Quit mid-batch, start again: the batch carries on and nothing is converted twice.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let before = m.get(&id).unwrap();
    assert_eq!(before.status, Status::Downloading, "the batch must still be running when DasIDM quits");
    assert!(before.convert.as_ref().unwrap().done < 6);
    m.shutdown().await;
    drop(m);
    let m2 = open(&data);
    wait_status(&m2, &id, Status::Done, 300).await;
    let it = m2.get(&id).unwrap();
    let c = it.convert.unwrap();
    assert_eq!((c.files, c.done, c.failed), (6, 6, 0));
    let out = album.join("Opus 160");
    let top: Vec<_> = std::fs::read_dir(&out).unwrap().flatten().filter(|e| e.path().is_file()).collect();
    let cd2: Vec<_> = std::fs::read_dir(out.join("CD2")).unwrap().flatten().collect();
    assert_eq!((top.len(), cd2.len()), (3, 3), "no duplicates like 'name (1).opus'");
    assert!(temps(tmp.path()).is_empty(), "left {:?}", temps(tmp.path()));
    m2.shutdown().await;
}

/// Labels name the output folders, so they must be distinct.
#[test]
fn preset_labels_are_distinct() {
    let mut labels: Vec<&str> = PRESETS.iter().map(|p| p.label()).collect();
    labels.sort();
    labels.dedup();
    assert_eq!(labels.len(), PRESETS.len());
    assert!(PRESETS.iter().any(|p| p.lossless()) && PRESETS.iter().any(|p| !p.lossless()));
}
