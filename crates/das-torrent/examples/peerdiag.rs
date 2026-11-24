//! What each peer delivers: a plain librqbit session on one torrent, reporting every 5 s how many
//! live peers actually send data and how fast. Options are switches for quick experiments.
//!
//!   cargo run --release -p das-torrent --example peerdiag -- <dir> <torrent url> [seconds]
//!       [utp] [peers=N] [window=N] [ct=SECONDS]
//!
//! Deletes the torrent and its files at the end.

use librqbit::api::{Api, TorrentIdOrHash};
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, ConnectionOptions, DhtSessionConfig, ListenerMode, ListenerOptions,
    PeerConnectionOptions, Session, SessionOptions,
};
use std::collections::HashMap;
use std::net::{Ipv6Addr, SocketAddr};
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let dir = std::path::PathBuf::from(&a[0]);
    let url = a[1].clone();
    let secs: u64 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(60);
    let flag = |name: &str| a.iter().any(|x| x == name);
    let num = |name: &str| a.iter().find_map(|x| x.strip_prefix(&format!("{name}=")).and_then(|v| v.parse::<u64>().ok()));
    let utp = flag("utp");
    let port = 40000 + (std::process::id() % 20000) as u16;

    let opts = SessionOptions {
        dht: Some(DhtSessionConfig { persistence: None, ..Default::default() }),
        persistence: None,
        listen: Some(ListenerOptions {
            mode: if utp { ListenerMode::TcpAndUtp } else { ListenerMode::TcpOnly },
            listen_addr: SocketAddr::from((Ipv6Addr::UNSPECIFIED, port)),
            enable_upnp_port_forwarding: true,
            ..Default::default()
        }),
        connect: Some(ConnectionOptions {
            peer_opts: Some(PeerConnectionOptions {
                connect_timeout: num("ct").map(Duration::from_secs),
                max_request_window: num("window").map(|n| n as usize),
                ..Default::default()
            }),
            ..Default::default()
        }),
        peer_limit: num("peers").map(|n| n as usize),
        ..Default::default()
    };
    println!("options: utp={utp} peers={:?} window={:?} connect_timeout={:?} port={port}", num("peers"), num("window"), num("ct"));
    let session = Session::new_with_opts(dir.join("out"), opts).await.expect("session");
    let t0 = Instant::now();
    let added = session
        .add_torrent(AddTorrent::from_url(url), Some(AddTorrentOptions { overwrite: true, ..Default::default() }))
        .await
        .expect("add");
    let AddTorrentResponse::Added(id, handle) = added else { panic!("not added") };
    println!("added in {:.1}s", t0.elapsed().as_secs_f64());
    let api = Api::new(session.clone(), None);

    let mut prev: HashMap<String, u64> = HashMap::new();
    let mut totals: Vec<f64> = Vec::new();
    let start = Instant::now();
    let mut tick = tokio::time::interval(Duration::from_secs(5));
    tick.tick().await;
    while start.elapsed().as_secs() < secs {
        tick.tick().await;
        let snap = match api.api_peer_stats(TorrentIdOrHash::Id(id), serde_json::from_str("{}").unwrap()) {
            Ok(s) => serde_json::to_value(&s).unwrap(),
            Err(_) => continue,
        };
        let peers = snap["peers"].as_object().cloned().unwrap_or_default();
        let mut rates: Vec<(f64, String, String)> = Vec::new();
        let (mut tcp, mut utp_n) = (0, 0);
        for (addr, p) in &peers {
            let fetched = p["counters"]["fetched_bytes"].as_u64().unwrap_or(0);
            let before = prev.insert(addr.clone(), fetched).unwrap_or(fetched);
            match p["conn_kind"].as_str() {
                Some("utp") => utp_n += 1,
                _ => tcp += 1,
            }
            rates.push((
                (fetched.saturating_sub(before)) as f64 / 5.0,
                addr.clone(),
                p["client_name"].as_str().unwrap_or("?").chars().take(18).collect(),
            ));
        }
        rates.sort_by(|x, y| y.0.total_cmp(&x.0));
        let total: f64 = rates.iter().map(|r| r.0).sum();
        totals.push(total);
        let sending: Vec<f64> = rates.iter().filter(|r| r.0 >= 16384.0 / 5.0).map(|r| r.0).collect();
        let median = sending.get(sending.len() / 2).copied().unwrap_or(0.0);
        let top: Vec<String> = rates.iter().take(5).map(|r| format!("{:.0}K {}", r.0 / 1e3, r.2)).collect();
        println!(
            "{:>4.0}s  {:>6.2} MB/s  live {:>3} (tcp {tcp}, utp {utp_n})  sending {:>3}  median {:>4.0} KB/s  top: {}",
            start.elapsed().as_secs_f64(),
            total / 1e6,
            peers.len(),
            sending.len(),
            median / 1e3,
            top.join(" | ")
        );
    }
    let steady: Vec<f64> = totals.iter().skip(2).copied().collect();
    let avg = steady.iter().sum::<f64>() / steady.len().max(1) as f64;
    println!("AVERAGE after 10 s: {:.2} MB/s", avg / 1e6);
    let _ = tokio::time::timeout(Duration::from_secs(10), session.delete(TorrentIdOrHash::Id(id), true)).await;
    drop(handle);
    std::process::exit(0);
}
