//! Speed benchmark through the real core: how fast a download gets going and how steady it stays.
//!
//!   cargo run --release -p das-core --example speedbench -- <data> <out> torrent <magnet or .torrent link> [seconds]
//!   cargo run --release -p das-core --example speedbench -- <data> <out> http <url> [seconds]
//!
//! Samples once a second until the time is up (default 90 s torrent, 30 s http) or 1 GB has
//! arrived, then removes the item and its files. Prints one line per second, a summary
//! (average, 10th percentile and minimum after a 10 s warm-up, peers, upload), and writes the
//! samples to <out>/speedbench.csv.

use das_core::model::{AddRequest, AddTorrentRequest, CoreEvent, Progress};
use das_core::Manager;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

const CAP_BYTES: u64 = 1_000_000_000;
const WARMUP_S: usize = 10;

#[tokio::main]
async fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (data, out, mode, link) = (&a[0], &a[1], a[2].as_str(), a[3].clone());
    let secs: u64 = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(if mode == "torrent" { 90 } else { 30 });

    let latest: Arc<Mutex<HashMap<String, Progress>>> = Arc::default();
    let sink = latest.clone();
    let core = Manager::open(
        std::path::Path::new(data),
        Arc::new(move |e| {
            if let CoreEvent::Progress { items, .. } = e {
                let mut m = sink.lock();
                for p in items {
                    m.insert(p.id.clone(), p);
                }
            }
        }),
    )
    .unwrap();
    let mut s = core.settings();
    s.download_dir = out.clone();
    core.set_settings(s).unwrap();
    tokio::spawn(core.clone().run());

    let t0 = Instant::now();
    let id = match mode {
        "torrent" => {
            let p = core.torrent_preview(Some(link.clone()), None).await.expect("preview");
            println!("torrent: {} ({:.2} GB, {} pieces of {} KiB), metadata in {:.1}s",
                p.name, p.size as f64 / 1e9, p.pieces, p.piece_size / 1024, t0.elapsed().as_secs_f64());
            core.add_torrent(AddTorrentRequest { info_hash: p.info_hash, only_files: None, extra_trackers: vec![], save_dir: Some(out.clone()), start: true })
                .await
                .expect("add torrent")
                .id
        }
        "http" => core
            .add(AddRequest { url: link.clone(), file_name: None, save_dir: Some(out.clone()), connections: None, headers: vec![], start: true, allow_page: false })
            .expect("add")
            .id,
        _ => panic!("mode is torrent or http"),
    };

    let started = Instant::now();
    let mut rows: Vec<(f64, u64, u64, u32, u32, usize)> = Vec::new(); // t, down B/s, up B/s, peers, seeds, lines
    // Bytes received, summed from the measured speed: a new torrent's `downloaded` first counts
    // the hash check of its freshly created files, which isn't data from the network.
    let mut received = 0f64;
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    tick.tick().await;
    loop {
        tick.tick().await;
        let t = started.elapsed().as_secs_f64();
        let p = latest.lock().get(&id).cloned();
        let item = core.get(&id);
        if let Some(p) = &p {
            received += p.speed as f64;
            let (up, peers, seeds) = p.torrent.as_ref().map(|t| (t.up_speed, t.peers, t.seeds)).unwrap_or((0, 0, 0));
            rows.push((t, p.speed, up, peers, seeds, p.active_connections));
            println!("{:>5.0}s  down {:>7.2} MB/s  up {:>6.2} MB/s  peers {:>3} (seeds {:>3})  lines {:>2}  got {:>7.1} MB",
                t, p.speed as f64 / 1e6, up as f64 / 1e6, peers, seeds, p.active_connections, received / 1e6);
        }
        let done = item.as_ref().is_some_and(|i| matches!(i.status, das_core::Status::Done | das_core::Status::Failed));
        if done || t >= secs as f64 || received >= CAP_BYTES as f64 {
            if let Some(i) = &item {
                if i.status == das_core::Status::Failed {
                    println!("FAILED: {:?}", i.error);
                }
            }
            break;
        }
    }
    core.remove(&[id.clone()], true);
    tokio::time::sleep(Duration::from_secs(2)).await;

    let steady: Vec<u64> = rows.iter().skip(WARMUP_S).map(|r| r.1).collect();
    let mut sorted = steady.clone();
    sorted.sort_unstable();
    let mb = |b: f64| b / 1e6;
    let avg = if steady.is_empty() { 0.0 } else { steady.iter().sum::<u64>() as f64 / steady.len() as f64 };
    let p10 = sorted.get(sorted.len() / 10).copied().unwrap_or(0) as f64;
    let min = sorted.first().copied().unwrap_or(0) as f64;
    let max = sorted.last().copied().unwrap_or(0) as f64;
    let first_mb = rows.iter().find(|r| r.1 >= 1_000_000).map(|r| r.0);
    let peers_avg = rows.iter().skip(WARMUP_S).map(|r| r.3 as f64).sum::<f64>() / steady.len().max(1) as f64;
    let peers_max = rows.iter().map(|r| r.3).max().unwrap_or(0);
    let up_avg = rows.iter().skip(WARMUP_S).map(|r| r.2 as f64).sum::<f64>() / steady.len().max(1) as f64;
    let up_max = rows.iter().map(|r| r.2).max().unwrap_or(0);
    println!();
    println!("SUMMARY {mode}: {} s sampled ({} after warm-up)", rows.len(), steady.len());
    println!("  down  avg {:.2}  p10 {:.2}  min {:.2}  max {:.2} MB/s   steadiness p10/avg {:.0}%",
        mb(avg), mb(p10), mb(min), mb(max), if avg > 0.0 { p10 / avg * 100.0 } else { 0.0 });
    println!("  first 1 MB/s at {}", first_mb.map(|t| format!("{t:.0} s")).unwrap_or("never".into()));
    if mode == "torrent" {
        println!("  peers avg {peers_avg:.0}  max {peers_max}   upload avg {:.2}  max {:.2} MB/s", mb(up_avg), mb(up_max as f64));
    }
    let csv: String = std::iter::once("t,down,up,peers,seeds,lines\n".to_string())
        .chain(rows.iter().map(|r| format!("{:.0},{},{},{},{},{}\n", r.0, r.1, r.2, r.3, r.4, r.5)))
        .collect();
    let _ = std::fs::write(std::path::Path::new(out).join("speedbench.csv"), csv);
    // A torrent session can take a long time to wind down; the numbers are already out.
    let _ = tokio::time::timeout(Duration::from_secs(5), core.shutdown()).await;
    std::process::exit(0);
}
