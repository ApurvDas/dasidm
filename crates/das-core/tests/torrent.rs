//! A torrent through the Manager: preview, add, finish into seeding, stop seeding, remove.
//! The swarm is local (librqbit seeder + a tiny HTTP tracker on 127.0.0.1, DHT off).

use axum::routing::get;
use das_core::model::AddTorrentRequest;
use das_core::{Manager, Settings, Status};
use librqbit::spawn_utils::BlockingSpawner;
use librqbit::{AddTorrent, AddTorrentOptions, CreateTorrentOptions, ListenerOptions, Session, SessionOptions};
use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

async fn wait_for(what: &str, secs: u64, mut ok: impl FnMut() -> bool) {
    let start = Instant::now();
    while !ok() {
        assert!(start.elapsed() < Duration::from_secs(secs), "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn torrent_item_lifecycle() {
    let tmp = tempfile::tempdir().unwrap();
    let seed = tmp.path().join("seed");
    std::fs::create_dir_all(&seed).unwrap();
    let data: Vec<u8> = (0..(1u32 << 20)).map(|i| (i.wrapping_mul(2654435761) >> 24) as u8).collect();
    std::fs::write(seed.join("song.flac"), &data).unwrap();

    // Tracker that always hands out the seeder.
    let seed_port = free_port();
    let peers: Vec<u8> = [127, 0, 0, 1].into_iter().chain(seed_port.to_be_bytes()).collect();
    let body: Arc<Vec<u8>> = Arc::new(
        [b"d8:completei1e10:incompletei0e8:intervali1e5:peers6:".as_slice(), &peers, b"e"].concat(),
    );
    let app = axum::Router::new().route("/announce", get(move || { let b = body.clone(); async move { b.to_vec() } }));
    let tl = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let tracker = format!("http://{}/announce", tl.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(tl, app).await.unwrap() });

    let made = librqbit::create_torrent(
        &seed.join("song.flac"),
        CreateTorrentOptions { name: None, trackers: vec![tracker], piece_length: Some(32 * 1024) },
        &BlockingSpawner::new(2),
    )
    .await
    .unwrap();
    let torrent_file = tmp.path().join("song.torrent");
    std::fs::write(&torrent_file, made.as_bytes().unwrap()).unwrap();
    let seeder = Session::new_with_opts(
        seed.clone(),
        SessionOptions {
            dht: None,
            disable_trackers: true,
            disable_local_service_discovery: true,
            persistence: None,
            ipv4_only: true,
            listen: Some(ListenerOptions { listen_addr: SocketAddr::from((Ipv4Addr::LOCALHOST, seed_port)), ipv4_only: true, ..Default::default() }),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    seeder
        .add_torrent(
            AddTorrent::from_bytes(made.as_bytes().unwrap()),
            Some(AddTorrentOptions { output_folder: Some(seed.to_string_lossy().into_owned()), overwrite: true, ..Default::default() }),
        )
        .await
        .unwrap();

    // DasIDM.
    // Stop the idle torrent session after 1.5 s instead of a minute.
    unsafe { std::env::set_var("DASIDM_TORRENT_IDLE_MS", "1500") };
    let dl = tmp.path().join("Downloads");
    let m = Manager::open(&tmp.path().join("data"), Arc::new(|_| {})).unwrap();
    let port = m.settings().torrent_port;
    assert!(port >= 20000, "a fixed torrent port is picked on first start");
    m.set_settings(Settings {
        download_dir: dl.to_string_lossy().into_owned(),
        torrent_dht: false,
        torrent_upnp: false,
        seed_ratio: 0.0,
        default_trackers: "udp://tracker.invalid:6969/announce\n\n".into(),
        ..m.settings()
    })
    .unwrap();
    tokio::spawn(m.clone().run());

    let p = m.torrent_preview(None, Some(torrent_file.to_string_lossy().into_owned())).await.unwrap();
    assert_eq!(p.files.len(), 1);
    let item = m
        .add_torrent(AddTorrentRequest { info_hash: p.info_hash.clone(), only_files: None, extra_trackers: vec![], save_dir: None, start: true })
        .await
        .unwrap();
    assert_eq!(item.category, "torrents");
    assert!(m.add_torrent(AddTorrentRequest { info_hash: p.info_hash.clone(), only_files: None, extra_trackers: vec![], save_dir: None, start: true }).await.is_err(), "no duplicates");

    wait_for("torrent done and seeding", 60, || {
        m.get(&item.id).is_some_and(|i| i.status == Status::Done && i.torrent.as_ref().is_some_and(|t| t.seeding))
    })
    .await;
    let done = m.get(&item.id).unwrap();
    let path = done.file_path.clone().unwrap();
    assert!(path.ends_with("Torrents\\song.flac"), "{path}");
    assert!(std::fs::read(&path).unwrap() == data, "bytes differ");
    // The default tracker from Settings was added to the torrent.
    assert!(m.torrent_trackers(&item.id).iter().any(|t| t.url.starts_with("udp://tracker.invalid")));

    // Pausing a seeding torrent stops seeding; it stays Done.
    m.pause(&[item.id.clone()]);
    let after = m.get(&item.id).unwrap();
    assert_eq!(after.status, Status::Done);
    assert!(!after.torrent.unwrap().seeding);
    // Nothing to download or seed: the session stops by itself, and Seed starts it again.
    wait_for("idle session stopped", 15, || !m.torrents_running()).await;
    m.seed(&[item.id.clone()]);
    assert!(m.get(&item.id).unwrap().torrent.unwrap().seeding);
    wait_for("session back for seeding", 15, || m.torrents_running()).await;
    assert!(m.torrent_trackers(&item.id).iter().any(|t| t.url.starts_with("udp://tracker.invalid")), "trackers kept across the stop");

    // Removing a finished torrent keeps its file unless asked.
    m.remove(&[item.id.clone()], false);
    assert!(m.get(&item.id).is_none());
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(std::path::Path::new(&path).exists());
    m.shutdown().await;
    seeder.stop().await;
}
