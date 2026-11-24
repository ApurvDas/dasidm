//! DasIDM torrents: one librqbit session, started only when a torrent needs it (an idle
//! DasIDM with no torrents keeps no DHT, sockets or peer tasks), plus qBittorrent-style
//! editable trackers through DasIDM's librqbit patch.

use anyhow::{Context, Result, bail};
use librqbit::api::TorrentIdOrHash;
use librqbit::limits::LimitsConfig;
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, ConnectionOptions, DhtSessionConfig, ListenerMode, ListenerOptions,
    ManagedTorrent, PeerConnectionOptions, Session, SessionOptions, SessionPersistenceConfig, TorrentStatsState, TrackerState,
};
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::net::{Ipv6Addr, SocketAddr};
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::atomic::{AtomicUsize, Ordering};

pub type Id20 = librqbit::dht::Id20;

/// Peers kept connected per torrent.
pub const PEER_LIMIT: usize = 200;
/// How long an outgoing connection may take before its slot goes to the next address.
pub const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

#[derive(Clone, Debug)]
pub struct TorrentSettings {
    /// Port for incoming peers (TCP). 0 picks one at random.
    pub port: u16,
    pub upnp: bool,
    pub dht: bool,
    /// Find peers on the local network too.
    pub lsd: bool,
    /// Talk uTP (BitTorrent over UDP) as well as TCP: much of a qBittorrent swarm uses it.
    pub utp: bool,
    /// Download / upload limits in bytes per second; 0 = none.
    pub down_bps: u64,
    pub up_bps: u64,
    /// Listen only on this address (tests use 127.0.0.1).
    pub bind: Option<SocketAddr>,
}

impl Default for TorrentSettings {
    fn default() -> Self {
        Self { port: 0, upnp: true, dht: true, lsd: true, utp: true, down_bps: 0, up_bps: 0, bind: None }
    }
}

/// Where a torrent comes from.
pub enum Source {
    /// A `magnet:` link, or an http(s) link to a .torrent file.
    Link(String),
    /// The bytes of a .torrent file.
    File(Vec<u8>),
}

impl Source {
    fn add(&self) -> AddTorrent<'_> {
        match self {
            Source::Link(l) => AddTorrent::from_url(l.as_str()),
            Source::File(b) => AddTorrent::from_bytes(b.clone()),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub index: usize,
    /// Path inside the torrent, with `\` between folders.
    pub path: String,
    pub size: u64,
    /// Bytes verified on disk (0 in a preview).
    pub done: u64,
}

/// What a torrent holds, read before anything is downloaded.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub info_hash: String,
    pub name: String,
    pub size: u64,
    pub files: Vec<FileEntry>,
    pub trackers: Vec<String>,
    pub private: bool,
    pub piece_size: u32,
    pub pieces: u32,
    /// The .torrent file itself, so adding it later needs no second metadata fetch.
    #[serde(skip)]
    pub torrent: Vec<u8>,
}

pub struct AddOptions {
    pub output_dir: PathBuf,
    /// Indexes of the files to download; None = all.
    pub only_files: Option<Vec<usize>>,
    /// Trackers added on top of the torrent's own (e.g. the user's default list).
    pub extra_trackers: Vec<String>,
    pub paused: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum State {
    /// Checking pieces already on disk.
    Checking,
    Downloading,
    /// Everything chosen is on disk; still uploading to others.
    Seeding,
    Paused,
    Error,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub state: State,
    pub name: Option<String>,
    /// Bytes of the chosen files on disk, and their total.
    pub downloaded: u64,
    pub total: u64,
    pub uploaded: u64,
    pub down_speed: u64,
    pub up_speed: u64,
    /// Connected peers, how many of them are seeds, and peers known in all.
    pub peers: u32,
    pub seeds: u32,
    pub known_peers: u32,
    pub finished: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackerView {
    pub url: String,
    pub tier: u32,
    /// notContacted, updating, working, notWorking, disabled
    pub state: &'static str,
    pub seeders: Option<u64>,
    pub leechers: Option<u64>,
    pub peers: u64,
    /// Milliseconds since 1970.
    pub last_announce: Option<i64>,
    pub next_announce: Option<i64>,
    pub message: Option<String>,
}

fn ms(t: Option<SystemTime>) -> Option<i64> {
    t.and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64)
}

fn nz(bps: u64) -> Option<NonZeroU32> {
    NonZeroU32::new(bps.min(u32::MAX as u64) as u32)
}

/// A tracker address people paste: trimmed, http(s) or udp only.
pub fn parse_tracker(s: &str) -> Option<url::Url> {
    let u = url::Url::parse(s.trim()).ok()?;
    matches!(u.scheme(), "http" | "https" | "udp").then_some(u)
}

pub struct Torrents {
    dir: PathBuf,
    settings: Mutex<TorrentSettings>,
    /// The running session, if any. It starts on first use and stops again when idle.
    session: parking_lot::RwLock<Option<Arc<Session>>>,
    /// Serialises starting and stopping.
    lifecycle: tokio::sync::Mutex<()>,
    /// Operations in flight (preview, add, resume…); the session is never stopped under one.
    busy: AtomicUsize,
    /// Tracker lists the user edited, by info hash (librqbit keeps only the original list).
    saved_trackers: Mutex<HashMap<String, Vec<String>>>,
}

impl Torrents {
    /// `dir` holds the session state, fast-resume data and the DHT table.
    pub fn new(dir: &Path, settings: TorrentSettings) -> Self {
        let saved = std::fs::read(dir.join("trackers.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        Self {
            dir: dir.to_path_buf(),
            settings: Mutex::new(settings),
            session: parking_lot::RwLock::new(None),
            lifecycle: tokio::sync::Mutex::new(()),
            busy: AtomicUsize::new(0),
            saved_trackers: Mutex::new(saved),
        }
    }

    /// Whether the session is running (it starts on first use).
    pub fn is_started(&self) -> bool {
        self.session.read().is_some()
    }

    fn current(&self) -> Option<Arc<Session>> {
        self.session.read().clone()
    }

    /// Mark an operation in flight until the guard is dropped.
    fn busy(&self) -> Busy<'_> {
        self.busy.fetch_add(1, Ordering::SeqCst);
        Busy(&self.busy)
    }

    /// Stop the session to free its memory, sockets and DHT (torrents keep their state on disk
    /// and come back on the next start). Does nothing while an operation is in flight; returns
    /// whether it stopped.
    pub async fn stop_if_idle(&self) -> bool {
        let _g = self.lifecycle.lock().await;
        if self.busy.load(Ordering::SeqCst) > 0 {
            return false;
        }
        let Some(s) = self.session.write().take() else { return false };
        s.stop().await;
        true
    }

    /// Start the session (and bring back the torrents it remembers).
    pub async fn start(&self) -> Result<Arc<Session>> {
        if let Some(s) = self.current() {
            return Ok(s);
        }
        let _g = self.lifecycle.lock().await;
        if let Some(s) = self.current() {
            return Ok(s);
        }
        let session = async {
                let s = self.settings.lock().clone();
                std::fs::create_dir_all(&self.dir)?;
                let listen = s.bind.unwrap_or_else(|| (Ipv6Addr::UNSPECIFIED, s.port).into());
                let opts = SessionOptions {
                    dht: s.dht.then(|| DhtSessionConfig {
                        persistence: Some(librqbit::dht::DhtPersistenceConfig {
                            config_filename: Some(self.dir.join("dht.json")),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    fastresume: true,
                    persistence: Some(SessionPersistenceConfig::Json { folder: Some(self.dir.join("session")) }),
                    listen: Some(ListenerOptions {
                        mode: if s.utp { ListenerMode::TcpAndUtp } else { ListenerMode::TcpOnly },
                        listen_addr: listen,
                        enable_upnp_port_forwarding: s.upnp,
                        ipv4_only: listen.is_ipv4(),
                        ..Default::default()
                    }),
                    // Peers to keep per torrent (librqbit's default is 128; libtorrent's 200).
                    peer_limit: Some(PEER_LIMIT),
                    connect: Some(ConnectionOptions {
                        peer_opts: Some(PeerConnectionOptions {
                            // A dead address holds a peer slot until this runs out.
                            connect_timeout: Some(CONNECT_TIMEOUT),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ratelimits: LimitsConfig { upload_bps: nz(s.up_bps), download_bps: nz(s.down_bps) },
                    disable_local_service_discovery: !s.lsd,
                    ipv4_only: listen.is_ipv4(),
                    ..Default::default()
                };
                let session = Session::new_with_opts(self.dir.join("unsorted"), opts).await.context("could not start torrents")?;
                self.apply_saved_trackers(&session);
                Ok::<_, anyhow::Error>(session)
        }
        .await?;
        *self.session.write() = Some(session.clone());
        Ok(session)
    }

    fn apply_saved_trackers(&self, session: &Arc<Session>) {
        let saved = self.saved_trackers.lock().clone();
        session.with_torrents(|all| {
            for (_, t) in all {
                let Some(list) = saved.get(&t.info_hash().as_string()) else { continue };
                let want: Vec<url::Url> = list.iter().filter_map(|u| url::Url::parse(u).ok()).collect();
                let set = &t.shared().tracker_set;
                let extra: Vec<url::Url> = set.urls().into_iter().filter(|u| !want.contains(u)).collect();
                set.remove(&extra);
                set.add(want, Some(0));
            }
        });
    }

    fn handle(&self, hash: &str) -> Option<Arc<ManagedTorrent>> {
        let id = Id20::from_str(hash).ok()?;
        self.current()?.get(TorrentIdOrHash::Hash(id))
    }

    /// Info hashes of every torrent the session knows.
    pub fn hashes(&self) -> Vec<String> {
        let Some(s) = self.current() else { return vec![] };
        s.with_torrents(|all| all.map(|(_, t)| t.info_hash().as_string()).collect())
    }

    /// Read a torrent's name and files without downloading it (a magnet asks peers first).
    pub async fn preview(&self, src: &Source) -> Result<Preview> {
        let _busy = self.busy();
        let session = self.start().await?;
        let opts = AddTorrentOptions { list_only: true, ..Default::default() };
        let AddTorrentResponse::ListOnly(r) = session.add_torrent(src.add(), Some(opts)).await? else {
            bail!("the torrent could not be read");
        };
        let info = r.info.info();
        let files: Vec<FileEntry> = r
            .info
            .iter_file_details()
            .enumerate()
            .map(|(index, f)| FileEntry { index, path: f.filename.to_vec().join("\\"), size: f.len, done: 0 })
            .collect();
        let meta = librqbit::torrent_from_bytes(&r.torrent_bytes).ok();
        let mut trackers: Vec<String> = Vec::new();
        if let Some(m) = &meta {
            for t in m.iter_announce() {
                let t = String::from_utf8_lossy(t.as_ref()).into_owned();
                if !trackers.contains(&t) {
                    trackers.push(t);
                }
            }
        }
        if let Source::Link(l) = src {
            if let Ok(m) = librqbit::Magnet::parse(l) {
                for t in m.trackers {
                    if !trackers.contains(&t) {
                        trackers.push(t);
                    }
                }
            }
        }
        Ok(Preview {
            info_hash: r.info_hash.as_string(),
            name: r.info.name().map(|n| n.into_owned()).unwrap_or_else(|| "torrent".into()),
            size: files.iter().map(|f| f.size).sum(),
            files,
            trackers,
            private: info.private,
            piece_size: info.piece_length,
            pieces: r.info.lengths().total_pieces(),
            torrent: r.torrent_bytes.to_vec(),
        })
    }

    /// Add a torrent; returns its info hash. Adding one that is already there returns its hash.
    pub async fn add(&self, src: &Source, o: AddOptions) -> Result<String> {
        let _busy = self.busy();
        let session = self.start().await?;
        let mut trackers: Vec<String> = o.extra_trackers.iter().filter(|t| parse_tracker(t).is_some()).cloned().collect();
        trackers.dedup();
        let opts = AddTorrentOptions {
            paused: o.paused,
            only_files: o.only_files,
            output_folder: Some(o.output_dir.to_string_lossy().into_owned()),
            overwrite: true, // resume into files already there (verified piece by piece)
            trackers: (!trackers.is_empty()).then_some(trackers),
            ..Default::default()
        };
        let handle = match session.add_torrent(src.add(), Some(opts)).await? {
            AddTorrentResponse::Added(_, h) | AddTorrentResponse::AlreadyManaged(_, h) => h,
            AddTorrentResponse::ListOnly(_) => bail!("unexpected list-only answer"),
        };
        // librqbit restores only the torrent's own trackers after a restart; keep the full list
        // (with the extra and default ones) so they come back too.
        if !self.saved_trackers.lock().contains_key(&handle.info_hash().as_string()) {
            self.remember_trackers(&handle);
        }
        Ok(handle.info_hash().as_string())
    }

    pub async fn pause(&self, hash: &str) -> Result<()> {
        let _busy = self.busy();
        let Some(session) = self.current() else { return Ok(()) }; // not running: nothing to pause
        let h = self.handle(hash).context("torrent not found")?;
        if matches!(h.stats().state, TorrentStatsState::Paused) {
            return Ok(());
        }
        session.pause(&h).await
    }

    pub async fn resume(&self, hash: &str) -> Result<()> {
        let _busy = self.busy();
        let session = self.start().await?;
        let h = self.handle(hash).context("torrent not found")?;
        if !matches!(h.stats().state, TorrentStatsState::Paused) {
            return Ok(());
        }
        session.unpause(&h).await
    }

    /// Forget a torrent; with `delete_files` its data goes too.
    pub async fn remove(&self, hash: &str, delete_files: bool) -> Result<()> {
        if self.saved_trackers.lock().remove(hash).is_some() {
            self.save_trackers();
        }
        let _busy = self.busy();
        let session = self.start().await?; // the session owns the files and their state
        let id = Id20::from_str(hash).map_err(|_| anyhow::anyhow!("bad info hash"))?;
        if session.get(TorrentIdOrHash::Hash(id)).is_none() {
            return Ok(());
        }
        session.delete(TorrentIdOrHash::Hash(id), delete_files).await
    }

    pub fn stats(&self, hash: &str) -> Option<Stats> {
        let h = self.handle(hash)?;
        let s = h.stats();
        let live = s.live.as_ref();
        let peers = live.map(|l| &l.snapshot.peer_stats);
        let mib = |m: f64| (m * 1024.0 * 1024.0).round() as u64;
        let state = match s.state {
            TorrentStatsState::Initializing { .. } => State::Checking,
            TorrentStatsState::Live if s.finished => State::Seeding,
            TorrentStatsState::Live => State::Downloading,
            TorrentStatsState::Paused => State::Paused,
            TorrentStatsState::Error => State::Error,
        };
        Some(Stats {
            state,
            name: h.name(),
            downloaded: s.progress_bytes,
            total: s.total_bytes,
            uploaded: s.uploaded_bytes,
            down_speed: live.map_or(0, |l| mib(l.download_speed.mbps)),
            up_speed: live.map_or(0, |l| mib(l.upload_speed.mbps)),
            peers: peers.map_or(0, |p| p.live),
            seeds: peers.map_or(0, |p| p.live_seeders),
            known_peers: peers.map_or(0, |p| p.seen),
            finished: s.finished,
            error: s.error,
        })
    }

    /// The torrent's files with how much of each is on disk.
    pub fn files(&self, hash: &str) -> Vec<FileEntry> {
        let Some(h) = self.handle(hash) else { return vec![] };
        let progress = h.stats().file_progress;
        h.with_metadata(|m| {
            m.info
                .iter_file_details()
                .enumerate()
                .map(|(index, f)| FileEntry {
                    index,
                    path: f.filename.to_vec().join("\\"),
                    size: f.len,
                    done: progress.get(index).copied().unwrap_or(0),
                })
                .collect()
        })
        .unwrap_or_default()
    }

    /// Indexes of the files being downloaded (None = all).
    pub fn chosen_files(&self, hash: &str) -> Option<Vec<usize>> {
        self.handle(hash)?.only_files()
    }

    /// Change which files are downloaded, while the torrent runs.
    pub async fn set_files(&self, hash: &str, only: Vec<usize>) -> Result<()> {
        let _busy = self.busy();
        let session = self.start().await?;
        let h = self.handle(hash).context("torrent not found")?;
        if only.is_empty() {
            bail!("choose at least one file");
        }
        session.update_only_files(&h, &only.into_iter().collect::<HashSet<_>>()).await
    }

    /// `buckets` fractions of verified pieces, for the piece map (None while checking).
    pub fn piece_map(&self, hash: &str, buckets: usize) -> Option<Vec<f32>> {
        self.handle(hash)?.live()?.piece_map(buckets)
    }

    pub fn trackers(&self, hash: &str) -> Vec<TrackerView> {
        let Some(h) = self.handle(hash) else { return vec![] };
        h.shared()
            .tracker_set
            .list()
            .into_iter()
            .map(|t| TrackerView {
                url: t.url.to_string(),
                tier: t.tier,
                state: match t.state {
                    TrackerState::NotContacted => "notContacted",
                    TrackerState::Updating => "updating",
                    TrackerState::Working => "working",
                    TrackerState::NotWorking => "notWorking",
                    TrackerState::Disabled => "disabled",
                },
                seeders: t.seeders,
                leechers: t.leechers,
                peers: t.peers_received,
                last_announce: ms(t.last_announce),
                next_announce: ms(t.next_announce),
                message: t.message,
            })
            .collect()
    }

    /// Add trackers (one per line or a list); returns how many were new. Bad lines are skipped.
    pub fn add_trackers(&self, hash: &str, urls: &[String]) -> Result<usize> {
        let h = self.handle(hash).context("torrent not found")?;
        let parsed: Vec<url::Url> = urls.iter().filter_map(|u| parse_tracker(u)).collect();
        let n = h.shared().tracker_set.add(parsed, None);
        self.remember_trackers(&h);
        Ok(n)
    }

    pub fn remove_trackers(&self, hash: &str, urls: &[String]) -> Result<usize> {
        let h = self.handle(hash).context("torrent not found")?;
        let parsed: Vec<url::Url> = urls.iter().filter_map(|u| url::Url::parse(u.trim()).ok()).collect();
        let n = h.shared().tracker_set.remove(&parsed);
        self.remember_trackers(&h);
        Ok(n)
    }

    pub fn replace_tracker(&self, hash: &str, old: &str, new: &str) -> Result<()> {
        let h = self.handle(hash).context("torrent not found")?;
        let old = url::Url::parse(old.trim()).context("not a tracker address")?;
        let new = parse_tracker(new).context("trackers start with http://, https:// or udp://")?;
        if !h.shared().tracker_set.replace(&old, new) {
            bail!("that tracker is not in the list, or the new address already is");
        }
        self.remember_trackers(&h);
        Ok(())
    }

    /// Ask every tracker for peers now.
    pub fn reannounce(&self, hash: &str) -> Result<()> {
        self.handle(hash).context("torrent not found")?.shared().tracker_set.reannounce();
        Ok(())
    }

    fn remember_trackers(&self, h: &Arc<ManagedTorrent>) {
        let list = h.shared().tracker_set.urls().into_iter().map(|u| u.to_string()).collect();
        self.saved_trackers.lock().insert(h.info_hash().as_string(), list);
        self.save_trackers();
    }

    fn save_trackers(&self) {
        let json = serde_json::to_vec_pretty(&*self.saved_trackers.lock()).unwrap_or_default();
        let _ = std::fs::create_dir_all(&self.dir);
        let tmp = self.dir.join("trackers.json.tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(tmp, self.dir.join("trackers.json"));
        }
    }

    /// New settings: limits apply at once; port, UPnP and DHT the next time the session starts.
    pub fn update_settings(&self, s: TorrentSettings) {
        let (down, up) = (s.down_bps, s.up_bps);
        *self.settings.lock() = s;
        self.set_limits(down, up);
    }

    /// Global limits in bytes per second (0 = none); applied live.
    pub fn set_limits(&self, down_bps: u64, up_bps: u64) {
        {
            let mut s = self.settings.lock();
            s.down_bps = down_bps;
            s.up_bps = up_bps;
        }
        if let Some(session) = self.current() {
            session.ratelimits.set_download_bps(nz(down_bps));
            session.ratelimits.set_upload_bps(nz(up_bps));
        }
    }

    /// Stop everything (the session saves its state as it goes).
    pub async fn shutdown(&self) {
        let _g = self.lifecycle.lock().await;
        // Take the session out first: in `if let`, the write guard would live through the
        // await, and every progress tick reading the session would block its thread.
        let session = self.session.write().take();
        if let Some(s) = session {
            s.stop().await;
        }
    }
}

struct Busy<'a>(&'a AtomicUsize);

impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}
