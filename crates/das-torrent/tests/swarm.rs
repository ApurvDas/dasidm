//! A whole swarm on 127.0.0.1: a librqbit seeder, DasIDM's `Torrents` as the downloader and
//! a tiny HTTP tracker. DHT and local discovery are off, so peers can only come from trackers.

use axum::extract::{Path as UrlPath, State};
use axum::routing::get;
use das_torrent::{AddOptions, Source, State as TState, TorrentSettings, Torrents};
use librqbit::spawn_utils::BlockingSpawner;
use librqbit::{AddTorrent, AddTorrentOptions, CreateTorrentOptions, ListenerMode, ListenerOptions, Session, SessionOptions};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

#[derive(Clone)]
struct Tracker {
    seeder: SocketAddrV4,
    announces: Arc<parking_lot::Mutex<HashMap<String, Arc<AtomicU32>>>>,
}

impl Tracker {
    fn count(&self, name: &str) -> u32 {
        self.announces.lock().get(name).map_or(0, |c| c.load(Ordering::SeqCst))
    }
}

/// Answers every announce with the seeder as the only peer, every second.
async fn announce(State(t): State<Tracker>, UrlPath(name): UrlPath<String>) -> Vec<u8> {
    t.announces.lock().entry(name).or_default().fetch_add(1, Ordering::SeqCst);
    let mut body = b"d8:completei1e10:incompletei0e8:intervali1e5:peers6:".to_vec();
    body.extend_from_slice(&t.seeder.ip().octets());
    body.extend_from_slice(&t.seeder.port().to_be_bytes());
    body.push(b'e');
    body
}

async fn wait_for(what: &str, secs: u64, mut ok: impl FnMut() -> bool) {
    let start = Instant::now();
    while !ok() {
        assert!(start.elapsed() < Duration::from_secs(secs), "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn settings(port: u16) -> TorrentSettings {
    TorrentSettings {
        port,
        upnp: false,
        dht: false,
        lsd: false,
        bind: Some(SocketAddr::from((Ipv4Addr::LOCALHOST, port))),
        ..Default::default()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn swarm_with_live_trackers() {
    let tmp = tempfile::tempdir().unwrap();
    let seed_dir = tmp.path().join("seed");
    let dl_dir = tmp.path().join("downloads");
    let state_dir = tmp.path().join("state");
    std::fs::create_dir_all(seed_dir.join("payload")).unwrap();

    // 3 MiB of varied bytes in two files.
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut data = vec![0u8; 3 << 20];
    for b in data.iter_mut() {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *b = x as u8;
    }
    std::fs::write(seed_dir.join("payload").join("a.bin"), &data[..2 << 20]).unwrap();
    std::fs::write(seed_dir.join("payload").join("b.bin"), &data[2 << 20..]).unwrap();

    // The tracker.
    let seed_port = free_port();
    let tracker = Tracker { seeder: SocketAddrV4::new(Ipv4Addr::LOCALHOST, seed_port), announces: Default::default() };
    let app = axum::Router::new().route("/{name}/announce", get(announce)).with_state(tracker.clone());
    let tl = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tracker_base = format!("http://{}", tl.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(tl, app).await.unwrap() });
    let tracker_a = format!("{tracker_base}/a/announce");
    let tracker_b = format!("{tracker_base}/b/announce");
    let dead = format!("http://127.0.0.1:{}/announce", free_port());

    // The torrent (tracker A only) and the seeder.
    let made = librqbit::create_torrent(
        &seed_dir.join("payload"),
        CreateTorrentOptions { name: None, trackers: vec![tracker_a.clone()], piece_length: Some(64 * 1024) },
        &BlockingSpawner::new(2),
    )
    .await
    .unwrap();
    let torrent = made.as_bytes().unwrap().to_vec();
    let seeder = Session::new_with_opts(
        seed_dir.clone(),
        SessionOptions {
            dht: None,
            disable_trackers: true,
            disable_local_service_discovery: true,
            persistence: None,
            ipv4_only: true,
            listen: Some(ListenerOptions {
                listen_addr: SocketAddr::from((Ipv4Addr::LOCALHOST, seed_port)),
                ipv4_only: true,
                ..Default::default()
            }),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    seeder
        .add_torrent(
            AddTorrent::from_bytes(torrent.clone()),
            Some(AddTorrentOptions { output_folder: Some(seed_dir.join("payload").to_string_lossy().into_owned()), overwrite: true, ..Default::default() }),
        )
        .await
        .unwrap();

    // The downloader: DasIDM's Torrents.
    let dl_port = free_port();
    let t = Torrents::new(&state_dir, settings(dl_port));
    assert!(!t.is_started(), "the session must not start before a torrent needs it");
    let preview = t.preview(&Source::File(torrent.clone())).await.unwrap();
    assert_eq!(preview.files.len(), 2);
    assert_eq!(preview.size, 3 << 20);
    assert_eq!(preview.trackers, [tracker_a.clone()]);
    let hash = t
        .add(
            &Source::File(torrent.clone()),
            AddOptions { output_dir: dl_dir.join("payload"), only_files: None, extra_trackers: vec![], paused: false },
        )
        .await
        .unwrap();
    assert_eq!(hash, preview.info_hash);

    // Arrives byte-exact, via tracker A.
    let start = Instant::now();
    let mut last = Instant::now();
    while !t.stats(&hash).is_some_and(|s| s.finished) {
        if last.elapsed() > Duration::from_secs(5) {
            last = Instant::now();
            let seeder_stats = seeder.with_torrents(|all| all.map(|(_, h)| format!("{:?}", h.stats().state)).collect::<Vec<_>>());
            eprintln!(
                "{:?}\n  trackers {:?}\n  announces a={} seeder {:?}",
                t.stats(&hash),
                t.trackers(&hash),
                tracker.count("a"),
                seeder_stats
            );
        }
        assert!(start.elapsed() < Duration::from_secs(40), "timed out waiting for download");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let mut got = std::fs::read(dl_dir.join("payload").join("a.bin")).unwrap();
    got.extend(std::fs::read(dl_dir.join("payload").join("b.bin")).unwrap());
    assert!(got == data, "downloaded bytes differ");
    assert_eq!(t.stats(&hash).unwrap().state, TState::Seeding);
    assert!(t.piece_map(&hash, 16).unwrap().iter().all(|f| *f == 1.0));
    let a = t.trackers(&hash).into_iter().find(|x| x.url == tracker_a).unwrap();
    assert_eq!(a.state, "working");
    assert_eq!(a.seeders, Some(1));
    assert!(a.peers >= 1);

    // A tracker added live is announced to; a dead one reads "not working".
    assert_eq!(t.add_trackers(&hash, &[tracker_b.clone(), dead.clone(), "not a url".into()]).unwrap(), 2);
    wait_for("tracker B announce", 10, || tracker.count("b") >= 1).await;
    wait_for("dead tracker marked", 10, || {
        t.trackers(&hash).iter().any(|x| x.url == dead && x.state == "notWorking" && x.message.is_some())
    })
    .await;

    // A removed tracker stops being asked.
    assert_eq!(t.remove_trackers(&hash, &[tracker_a.clone()]).unwrap(), 1);
    tokio::time::sleep(Duration::from_millis(300)).await;
    let (a_then, b_then) = (tracker.count("a"), tracker.count("b"));
    t.reannounce(&hash).unwrap();
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert_eq!(tracker.count("a"), a_then, "removed tracker was still announced to");
    assert!(tracker.count("b") > b_then, "remaining tracker stopped announcing");

    // Edit an address in place.
    let b2 = format!("{tracker_base}/b2/announce");
    t.replace_tracker(&hash, &tracker_b, &b2).unwrap();
    wait_for("edited tracker announce", 10, || tracker.count("b2") >= 1).await;

    // After a restart the torrent comes back finished, with the edited tracker list.
    t.shutdown().await;
    drop(t);
    let t = Torrents::new(&state_dir, settings(free_port()));
    t.start().await.unwrap();
    assert_eq!(t.hashes(), [hash.clone()]);
    wait_for("restored torrent", 30, || t.stats(&hash).is_some_and(|s| s.finished)).await;
    let urls: Vec<String> = t.trackers(&hash).into_iter().map(|x| x.url).collect();
    assert!(urls.contains(&b2) && urls.contains(&dead) && !urls.contains(&tracker_a), "{urls:?}");

    // Stopping an idle session frees it; the next use starts it again (same port) with the
    // torrent and its edited trackers intact. A busy session is never stopped.
    assert!(t.stop_if_idle().await);
    assert!(!t.is_started());
    assert!(t.hashes().is_empty() && t.stats(&hash).is_none());
    t.start().await.unwrap();
    assert_eq!(t.hashes(), [hash.clone()]);
    wait_for("restored after idle stop", 30, || t.stats(&hash).is_some_and(|s| s.finished)).await;
    assert!(t.trackers(&hash).iter().any(|x| x.url == b2));
    let src = Source::File(torrent.clone());
    let (busy, stop) = tokio::join!(t.preview(&src), async {
        tokio::time::sleep(Duration::from_millis(5)).await;
        t.stop_if_idle().await
    });
    assert!(busy.is_ok(), "a preview must not be cut off by an idle stop");
    if stop {
        // It stopped once the preview was done; starting again re-checks pieces on disk.
        t.start().await.unwrap();
    }
    wait_for("finished after restart", 30, || t.stats(&hash).is_some_and(|s| s.finished)).await;

    // Files can be listed, and removal forgets the torrent but keeps its files unless asked.
    assert_eq!(t.files(&hash).iter().map(|f| f.done).sum::<u64>(), 3 << 20);
    t.remove(&hash, false).await.unwrap();
    assert!(t.hashes().is_empty());
    assert!(dl_dir.join("payload").join("a.bin").exists());
    t.shutdown().await;
    seeder.stop().await;
}

/// One file from a seeder that listens in `mode` to DasIDM's `Torrents` (uTP on); how long it took.
async fn transfer_over(mode: ListenerMode, size: usize) -> Duration {
    let tmp = tempfile::tempdir().unwrap();
    let seed_dir = tmp.path().join("seed");
    std::fs::create_dir_all(&seed_dir).unwrap();
    let mut x: u64 = 0x2545_F491_4F6C_DD1D;
    let data: Vec<u8> = (0..size).map(|_| { x ^= x << 13; x ^= x >> 7; x ^= x << 17; x as u8 }).collect();
    std::fs::write(seed_dir.join("big.bin"), &data).unwrap();

    let seed_port = free_port();
    let tracker = Tracker { seeder: SocketAddrV4::new(Ipv4Addr::LOCALHOST, seed_port), announces: Default::default() };
    let app = axum::Router::new().route("/{name}/announce", get(announce)).with_state(tracker);
    let tl = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tracker_url = format!("http://{}/t/announce", tl.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(tl, app).await.unwrap() });
    let made = librqbit::create_torrent(
        &seed_dir.join("big.bin"),
        CreateTorrentOptions { name: None, trackers: vec![tracker_url], piece_length: Some(256 * 1024) },
        &BlockingSpawner::new(2),
    )
    .await
    .unwrap();
    let torrent = made.as_bytes().unwrap().to_vec();
    let seeder = Session::new_with_opts(
        seed_dir.clone(),
        SessionOptions {
            dht: None,
            disable_trackers: true,
            disable_local_service_discovery: true,
            persistence: None,
            ipv4_only: true,
            listen: Some(ListenerOptions { mode, listen_addr: SocketAddr::from((Ipv4Addr::LOCALHOST, seed_port)), ipv4_only: true, ..Default::default() }),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    seeder
        .add_torrent(
            AddTorrent::from_bytes(torrent.clone()),
            Some(AddTorrentOptions { output_folder: Some(seed_dir.to_string_lossy().into_owned()), overwrite: true, ..Default::default() }),
        )
        .await
        .unwrap();

    let t = Torrents::new(&tmp.path().join("state"), settings(free_port()));
    let start = Instant::now();
    let hash = t
        .add(&Source::File(torrent), AddOptions { output_dir: tmp.path().join("dl"), only_files: None, extra_trackers: vec![], paused: false })
        .await
        .unwrap();
    wait_for("the transfer", 60, || t.stats(&hash).is_some_and(|s| s.finished)).await;
    let took = start.elapsed();
    assert!(std::fs::read(tmp.path().join("dl").join("big.bin")).unwrap() == data, "bytes differ");
    t.shutdown().await;
    seeder.stop().await;
    took
}

/// uTP carries a whole torrent on Windows, and isn't far slower than TCP.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn utp_moves_a_torrent() {
    let size = 48 << 20;
    let utp = transfer_over(ListenerMode::UtpOnly, size).await;
    let tcp = transfer_over(ListenerMode::TcpOnly, size).await;
    let mb = |d: Duration| size as f64 / 1e6 / d.as_secs_f64();
    eprintln!("48 MiB on 127.0.0.1: uTP {utp:.1?} ({:.0} MB/s), TCP {tcp:.1?} ({:.0} MB/s)", mb(utp), mb(tcp));
    assert!(utp < tcp * 4 + Duration::from_secs(2), "uTP far slower than TCP: {utp:?} vs {tcp:?}");
}
