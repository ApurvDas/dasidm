//! Manual end-to-end check of video links through the real core (network + yt-dlp).
//!
//!   cargo run -p das-core --example videotest -- <data> <out> install
//!   cargo run -p das-core --example videotest -- <data> <out> info <url>
//!   cargo run -p das-core --example videotest -- <data> <out> get <url> <label> [compat|best] [pause]

use das_core::model::{AddVideoRequest, CoreEvent};
use das_core::{Manager, Status};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (data, out, cmd) = (&a[0], &a[1], a[2].as_str());
    let core = Manager::open(
        std::path::Path::new(data),
        Arc::new(|e| {
            if let CoreEvent::Tools { status } = e {
                if let Some(i) = status.installing {
                    eprint!("\r  {} {}/{}  {:>6.1} MB", i.step, i.index + 1, i.count, i.downloaded as f64 / 1e6);
                }
            }
        }),
    )
    .unwrap();
    let mut s = core.settings();
    s.download_dir = out.clone();
    core.set_settings(s).unwrap();
    tokio::spawn(core.clone().run());

    match cmd {
        "install" => {
            let t = Instant::now();
            core.install_tools().await.expect("install");
            eprintln!();
            println!("installed in {:.1}s: {:?}", t.elapsed().as_secs_f64(), core.tools_status());
        }
        "info" => {
            let t = Instant::now();
            let v = core.video_info(a[3].clone()).await.expect("info");
            println!("{:.1}s  {} | {:?} | {:?}s | playlist {:?}", t.elapsed().as_secs_f64(), v.title, v.channel, v.duration, v.playlist);
            for o in &v.options {
                println!("  {:<6} {:<5} {:>10} {}{}  [{}]", o.label, o.ext, o.size.map(|s| format!("{:.1} MB", s as f64 / 1e6)).unwrap_or_default(),
                    o.detail, if o.recommended { "  *" } else { "" }, o.selector);
            }
        }
        "get" => {
            let v = core.video_info(a[3].clone()).await.expect("info");
            let want_compat = a.get(5).map(|s| s.as_str()) != Some("best");
            let opt = v
                .options
                .iter()
                .find(|o| o.label == a[4] && (o.ext == "mp4") == want_compat || o.label == a[4] && matches!(o.choice, das_core::video::Choice::Audio { .. }))
                .cloned()
                .expect("no such option");
            println!("chose {} {} ({})", opt.label, opt.ext, opt.detail);
            let pause = a.iter().any(|x| x == "pause");
            let items = core
                .add_video(AddVideoRequest {
                    url: v.url.clone(), option: opt, title: v.title.clone(), channel: v.channel.clone(), duration: v.duration,
                    thumbnail: v.thumbnail.clone(), site: Some(v.site.clone()), save_dir: Some(out.clone()), start: true, playlist: false,
                })
                .await
                .expect("add");
            let id = items[0].id.clone();
            let t = Instant::now();
            let mut paused_once = false;
            loop {
                tokio::time::sleep(Duration::from_millis(500)).await;
                let it = core.get(&id).unwrap();
                eprint!("\r  {:?} {:>7.1}/{:>7.1} MB   ", it.status, it.downloaded as f64 / 1e6, it.size.unwrap_or(0) as f64 / 1e6);
                if pause && !paused_once && it.downloaded > 0 && it.downloaded * 3 > it.size.unwrap_or(u64::MAX) {
                    eprintln!("\n  pausing at {:.1} MB", it.downloaded as f64 / 1e6);
                    core.pause(&[id.clone()]);
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    let p = core.get(&id).unwrap();
                    let work = std::path::Path::new(out).join(".dasidm").join(&id);
                    let kept: u64 = std::fs::read_dir(&work).map(|d| d.filter_map(|e| e.ok()).filter_map(|e| e.metadata().ok()).map(|m| m.len()).sum()).unwrap_or(0);
                    eprintln!("  status after pause: {:?}, kept in the work folder: {:.1} MB", p.status, kept as f64 / 1e6);
                    core.resume(&[id.clone()]);
                    paused_once = true;
                    let before = core.get(&id).unwrap().downloaded;
                    for _ in 0..40 {
                        tokio::time::sleep(Duration::from_millis(250)).await;
                        let r = core.get(&id).unwrap();
                        if r.downloaded != before {
                            eprintln!("  first report after resume: {:.1} MB (was {:.1} MB)", r.downloaded as f64 / 1e6, before as f64 / 1e6);
                            break;
                        }
                    }
                    continue;
                }
                match it.status {
                    Status::Done => {
                        let path = it.file_path.clone().unwrap();
                        println!("\ndone in {:.1}s: {} ({:.1} MB) thumb={}", t.elapsed().as_secs_f64(), path,
                            std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) as f64 / 1e6, it.media.as_ref().is_some_and(|m| m.thumb));
                        break;
                    }
                    Status::Failed => {
                        println!("\nFAILED: {:?}", it.error);
                        std::process::exit(1);
                    }
                    _ => {}
                }
            }
        }
        _ => panic!("unknown command"),
    }
    core.shutdown().await;
}
