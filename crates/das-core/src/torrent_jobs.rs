//! Torrent items: added after a preview, run by the BitTorrent session (not DasIDM's HTTP
//! queue), mapped onto Downloading / Done + seeding / Done / Paused / Failed.

use super::*;
use das_torrent::{AddOptions, FileEntry, Preview, Source, State as TState, TorrentSettings, TrackerView};

/// How long the torrent session may sit with nothing to do before it is stopped (a minute;
/// `DASIDM_TORRENT_IDLE_MS` overrides it for tests).
fn torrents_idle_ms() -> i64 {
    std::env::var("DASIDM_TORRENT_IDLE_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(60_000)
}

/// Where "Find more peers" gets public trackers: ngosang/trackerslist's "best" list, a widely
/// used list of public trackers that is checked and updated every day. Fetched only when the
/// user asks for it.
pub const PUBLIC_TRACKERS_URL: &str = "https://raw.githubusercontent.com/ngosang/trackerslist/master/trackers_best.txt";

/// The trackers in a list: one address per line; blank lines, comments and anything that is
/// not an http(s) or udp tracker are dropped, duplicates too, and at most 100 are kept.
pub fn parse_tracker_list(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        if das_torrent::parse_tracker(line).is_some() && !out.iter().any(|t| t == line) {
            out.push(line.to_string());
        }
        if out.len() == 100 {
            break;
        }
    }
    out
}

/// How many verified-piece buckets the pass and the torrent view draw.
const PIECE_BUCKETS: usize = 96;

pub(super) fn torrent_settings(s: &Settings) -> TorrentSettings {
    TorrentSettings {
        port: s.torrent_port,
        upnp: s.torrent_upnp,
        dht: s.torrent_dht,
        lsd: true,
        utp: s.torrent_utp,
        down_bps: s.speed_limit,
        up_bps: s.upload_limit,
        bind: None,
    }
}

/// A link (magnet: or http(s) .torrent) or a local .torrent file.
pub fn is_torrent_link(s: &str) -> bool {
    let s = s.trim();
    s.to_ascii_lowercase().starts_with("magnet:?")
        || (s.to_ascii_lowercase().ends_with(".torrent") && (Path::new(s).is_file() || s.starts_with("http")))
}

impl Manager {
    /// Read a torrent's name and files. `link` is a magnet or http(s) .torrent link, `path` a
    /// local .torrent file. A magnet asks peers for the file list, which can take a while.
    pub async fn torrent_preview(&self, link: Option<String>, path: Option<String>) -> Result<Preview, String> {
        let src = match (link.filter(|l| !l.trim().is_empty()), path) {
            (Some(l), _) => Source::Link(l.trim().to_string()),
            (None, Some(p)) => Source::File(std::fs::read(&p).map_err(|e| format!("Could not read {p}: {e}"))?),
            _ => return Err("Give a magnet link or a .torrent file".into()),
        };
        let preview = tokio::time::timeout(Duration::from_secs(120), self.inner.torrents.preview(&src))
            .await
            .map_err(|_| "No peer sent the file list within 2 minutes. Check the link, or try again later.".to_string())?
            .map_err(|e| friendly_torrent_error(&e))?;
        let mut cache = self.inner.previews.lock();
        if cache.len() > 16 {
            cache.clear();
        }
        cache.insert(preview.info_hash.clone(), Arc::new(preview.clone()));
        Ok(preview)
    }

    /// Add a previewed torrent with the chosen files.
    pub async fn add_torrent(&self, req: AddTorrentRequest) -> Result<ItemView, String> {
        let p = self
            .inner
            .previews
            .lock()
            .get(&req.info_hash)
            .cloned()
            .ok_or("Read the torrent again first (its file list was not kept)")?;
        if self.inner.state.lock().items.values().any(|i| i.torrent.as_ref().is_some_and(|t| t.info_hash == p.info_hash)) {
            return Err("This torrent is already in the list".into());
        }
        let only = req.only_files.filter(|o| o.len() < p.files.len());
        if only.as_ref().is_some_and(Vec::is_empty) {
            return Err("Choose at least one file".into());
        }
        let size: u64 = p.files.iter().filter(|f| only.as_ref().is_none_or(|o| o.contains(&f.index))).map(|f| f.size).sum();
        let settings = self.settings();
        let base = req.save_dir.filter(|d| !d.trim().is_empty()).map(PathBuf::from).unwrap_or_else(|| {
            let base = PathBuf::from(&settings.download_dir);
            match (settings.category_folders, folder_for("torrents")) {
                (true, Some(f)) => base.join(f),
                _ => base,
            }
        });
        // A many-file torrent gets a folder of its own, as in other clients.
        let output = if p.files.len() > 1 { base.join(sanitize_file_name(&p.name)) } else { base.clone() };
        let mut trackers = req.extra_trackers.clone();
        trackers.extend(settings.default_tracker_list());

        // Keep the .torrent, so the torrent can be added back if the session state is lost.
        let files_dir = self.inner.data_dir.join("torrents").join("files");
        let _ = std::fs::create_dir_all(&files_dir);
        let _ = std::fs::write(files_dir.join(format!("{}.torrent", p.info_hash)), &p.torrent);

        let hash = self
            .inner
            .torrents
            .add(
                &Source::File(p.torrent.clone()),
                AddOptions { output_dir: output.clone(), only_files: only.clone(), extra_trackers: trackers, paused: !req.start },
            )
            .await
            .map_err(|e| friendly_torrent_error(&e))?;
        let single = p.files.len() == 1;
        let view = {
            let mut st = self.inner.state.lock();
            let position = st.next_position;
            st.next_position += 1;
            let item = Item {
                id: format!("{:x}{:04x}", now_ms(), fastrand::u16(..)),
                url: format!("magnet:?xt=urn:btih:{hash}"),
                file_name: if single { p.files[0].path.rsplit('\\').next().unwrap_or(&p.name).to_string() } else { p.name.clone() },
                save_dir: base.to_string_lossy().into_owned(),
                file_path: None,
                size: Some(size),
                downloaded: 0,
                status: if req.start { Status::Downloading } else { Status::Paused },
                category: "torrents".into(),
                added_at: now_ms(),
                finished_at: None,
                error: None,
                connections: 0,
                resumable: true,
                headers: vec![],
                resume: None,
                position,
                name_fixed: true,
                allow_html: false,
                kind: Kind::Torrent,
                video: None,
                convert: None,
                torrent: Some(TorrentSpec {
                    info_hash: hash,
                    name: p.name.clone(),
                    files: p.files.len(),
                    private: p.private,
                    only_files: only,
                    output_dir: output.to_string_lossy().into_owned(),
                    uploaded: 0,
                    seeding: false,
                    seed_since: None,
                    finished_once: false,
                }),
            };
            let _ = self.inner.db.lock().save_item(&item);
            let view = ItemView::from(&item);
            st.items.insert(item.id.clone(), item);
            view
        };
        (self.inner.emit)(CoreEvent::Upsert { item: view.clone() });
        Ok(view)
    }

    fn hash_of(&self, id: &str) -> Result<String, String> {
        self.inner.torrent_hash(id).ok_or_else(|| "Not a torrent".to_string())
    }

    pub fn torrent_trackers(&self, id: &str) -> Vec<TrackerView> {
        self.inner.viewing_torrent();
        self.inner.torrent_hash(id).map(|h| self.inner.torrents.trackers(&h)).unwrap_or_default()
    }

    pub fn add_trackers(&self, id: &str, urls: Vec<String>) -> Result<usize, String> {
        let h = self.hash_of(id)?;
        let urls: Vec<String> = urls.iter().flat_map(|u| u.split_whitespace()).map(str::to_string).collect();
        self.inner.torrents.add_trackers(&h, &urls).map_err(|e| e.to_string())
    }

    pub fn remove_trackers(&self, id: &str, urls: Vec<String>) -> Result<usize, String> {
        let h = self.hash_of(id)?;
        self.inner.torrents.remove_trackers(&h, &urls).map_err(|e| e.to_string())
    }

    pub fn replace_tracker(&self, id: &str, old: String, new: String) -> Result<(), String> {
        let h = self.hash_of(id)?;
        self.inner.torrents.replace_tracker(&h, &old, &new).map_err(|e| e.to_string())
    }

    pub fn reannounce(&self, id: &str) -> Result<(), String> {
        let h = self.hash_of(id)?;
        self.inner.torrents.reannounce(&h).map_err(|e| e.to_string())
    }

    /// Today's public tracker list (see [`PUBLIC_TRACKERS_URL`]).
    pub async fn public_trackers(&self) -> Result<Vec<String>, String> {
        let (bytes, _) = self
            .inner
            .engine
            .fetch_small(PUBLIC_TRACKERS_URL, 256 * 1024)
            .await
            .map_err(|e| format!("Couldn't get the public tracker list: {e}"))?;
        let list = parse_tracker_list(&String::from_utf8_lossy(&bytes));
        if list.is_empty() {
            return Err("The public tracker list came back empty; try again later.".into());
        }
        Ok(list)
    }

    /// Add the public trackers to a torrent and ask every tracker for peers at once.
    /// Returns (newly added, in the list).
    pub async fn find_more_peers(&self, id: &str) -> Result<(usize, usize), String> {
        self.hash_of(id)?;
        let list = self.public_trackers().await?;
        let added = self.add_trackers(id, list.clone())?;
        let _ = self.reannounce(id);
        Ok((added, list.len()))
    }

    /// The torrent's files with progress, and which are chosen.
    pub fn torrent_files(&self, id: &str) -> (Vec<FileEntry>, Option<Vec<usize>>) {
        self.inner.viewing_torrent();
        let Some(h) = self.inner.torrent_hash(id) else { return (vec![], None) };
        (self.inner.torrents.files(&h), self.inner.torrents.chosen_files(&h))
    }

    /// Change which files are downloaded.
    pub async fn set_torrent_files(&self, id: &str, only: Vec<usize>) -> Result<(), String> {
        let h = self.hash_of(id)?;
        self.inner.torrents.set_files(&h, only.clone()).await.map_err(|e| e.to_string())?;
        let files = self.inner.torrents.files(&h);
        let item = {
            let mut st = self.inner.state.lock();
            let Some(item) = st.items.get_mut(id) else { return Ok(()) };
            let all = files.len();
            item.size = Some(files.iter().filter(|f| only.contains(&f.index)).map(|f| f.size).sum());
            if let Some(t) = item.torrent.as_mut() {
                t.only_files = (only.len() < all).then_some(only);
                // More files to fetch: it is downloading again.
                if item.status == Status::Done && files.iter().any(|f| t.only_files.as_ref().is_none_or(|o| o.contains(&f.index)) && f.done < f.size) {
                    item.status = Status::Downloading;
                    t.finished_once = false;
                    t.seeding = false;
                }
            }
            item.clone()
        };
        self.inner.persist_and_emit(&[item]);
        Ok(())
    }

    /// Whether the BitTorrent session is running right now.
    pub fn torrents_running(&self) -> bool {
        self.inner.torrents.is_started()
    }

    /// Upload a finished torrent to others again.
    pub fn seed(&self, ids: &[String]) {
        let mut changed = Vec::new();
        {
            let mut st = self.inner.state.lock();
            for id in ids {
                let Some(item) = st.items.get_mut(id) else { continue };
                let Some(t) = item.torrent.as_mut() else { continue };
                if item.status != Status::Done || t.seeding {
                    continue;
                }
                t.seeding = true;
                t.seed_since = Some(now_ms());
                let h = t.info_hash.clone();
                let tor = self.inner.torrents.clone();
                tokio::spawn(async move { let _ = tor.resume(&h).await; });
                changed.push(item.clone());
            }
        }
        self.inner.persist_and_emit(&changed);
    }
}

impl Inner {
    pub(super) fn torrent_hash(&self, id: &str) -> Option<String> {
        self.state.lock().items.get(id)?.torrent.as_ref().map(|t| t.info_hash.clone())
    }

    /// Start the session when a torrent needs it, and put back any torrent it lost.
    pub(super) fn start_torrents_if_needed(self: &Arc<Self>) {
        let wanted: Vec<(String, bool)> = self
            .state
            .lock()
            .items
            .values()
            .filter_map(|i| {
                let t = i.torrent.as_ref()?;
                Some((t.info_hash.clone(), i.status == Status::Downloading || (i.status == Status::Done && t.seeding)))
            })
            .collect();
        if !wanted.iter().any(|(_, active)| *active) {
            return; // nothing to run: keep the session (and its memory) off
        }
        let me = self.clone();
        tokio::spawn(async move {
            if let Err(e) = me.torrents.start().await {
                tracing::warn!("torrents did not start: {e:#}");
                return;
            }
            let known = me.torrents.hashes();
            for (hash, active) in wanted {
                if known.contains(&hash) {
                    if active {
                        let _ = me.torrents.resume(&hash).await;
                    }
                    continue;
                }
                me.readd_torrent(&hash, !active).await;
            }
        });
    }

    async fn readd_torrent(&self, hash: &str, paused: bool) {
        let Ok(bytes) = std::fs::read(self.data_dir.join("torrents").join("files").join(format!("{hash}.torrent"))) else { return };
        let spec = self.state.lock().items.values().find_map(|i| i.torrent.clone().filter(|t| t.info_hash == hash));
        let Some(spec) = spec else { return };
        let _ = self
            .torrents
            .add(
                &Source::File(bytes),
                AddOptions { output_dir: PathBuf::from(&spec.output_dir), only_files: spec.only_files.clone(), extra_trackers: vec![], paused },
            )
            .await;
    }

    pub(super) fn pause_torrent(self: &Arc<Self>, item: &mut Item) -> bool {
        let Some(t) = item.torrent.as_mut() else { return false };
        match item.status {
            Status::Downloading => item.status = Status::Paused,
            Status::Done if t.seeding => t.seeding = false,
            _ => return false,
        }
        let (h, tor) = (t.info_hash.clone(), self.torrents.clone());
        tokio::spawn(async move { let _ = tor.pause(&h).await; });
        true
    }

    pub(super) fn resume_torrent(self: &Arc<Self>, item: &mut Item) -> bool {
        let Some(t) = item.torrent.as_ref() else { return false };
        if !matches!(item.status, Status::Paused | Status::Failed) {
            return false;
        }
        item.status = Status::Downloading;
        item.error = None;
        let hash = t.info_hash.clone();
        let me = self.clone();
        tokio::spawn(async move {
            if me.torrents.start().await.is_err() {
                return;
            }
            if !me.torrents.hashes().contains(&hash) {
                me.readd_torrent(&hash, false).await;
            } else if let Err(e) = me.torrents.resume(&hash).await {
                tracing::warn!("resume torrent: {e:#}");
            }
        });
        true
    }

    pub(super) fn remove_torrent(self: &Arc<Self>, item: &Item, delete_files: bool) {
        let Some(t) = item.torrent.as_ref() else { return };
        // Unfinished data is always deleted, as for other downloads.
        let delete = delete_files || item.status != Status::Done;
        let (h, tor) = (t.info_hash.clone(), self.torrents.clone());
        let _ = std::fs::remove_file(self.data_dir.join("torrents").join("files").join(format!("{h}.torrent")));
        let dir = PathBuf::from(&t.output_dir);
        let many = t.files > 1;
        tokio::spawn(async move {
            let _ = tor.remove(&h, delete).await;
            if delete && many {
                let _ = std::fs::remove_dir(&dir); // only if empty
            }
        });
    }

    /// Live numbers for running torrents, folded into the regular progress tick.
    pub(super) fn torrent_progress(
        &self,
        items: &mut HashMap<String, Item>,
        visible: bool,
        out: &mut Vec<Progress>,
        totals: &mut Totals,
        changed: &mut Vec<Item>,
        completed: &mut Vec<Item>,
    ) {
        if !self.torrents.is_started() {
            return;
        }
        let settings = self.settings.read().clone();
        for item in items.values_mut() {
            // Work on the spec beside the item, then put it back before recording changes.
            let Some(mut t) = item.torrent.take() else { continue };
            let (was_changed, was_completed) = self.tick_one(item, &mut t, &settings, visible, out, totals);
            item.torrent = Some(t);
            if was_changed {
                changed.push(item.clone());
            }
            if was_completed {
                completed.push(item.clone());
            }
        }
    }

    /// One torrent's tick; returns (changed, just finished).
    fn tick_one(
        &self,
        item: &mut Item,
        t: &mut TorrentSpec,
        settings: &Settings,
        visible: bool,
        out: &mut Vec<Progress>,
        totals: &mut Totals,
    ) -> (bool, bool) {
        let active = item.status == Status::Downloading || (item.status == Status::Done && t.seeding);
        if !active {
            return (false, false);
        }
        let Some(s) = self.torrents.stats(&t.info_hash) else { return (false, false) };
        if s.state == TState::Error {
            item.status = Status::Failed;
            item.error = Some(s.error.as_deref().map(friendly_torrent_text).unwrap_or_else(|| "The torrent stopped with an error".into()));
            return (true, false);
        }
        let (mut changed, mut completed) = (false, false);
        // While the files are checked, librqbit counts the bytes checked so far; a new torrent
        // would briefly show gigabytes "done". Keep the last real figure until the check ends.
        if s.state != TState::Checking {
            item.downloaded = s.downloaded;
        }
        if s.total > 0 {
            item.size = Some(s.total);
        }
        t.uploaded = s.uploaded;
        if s.finished && !t.finished_once {
            t.finished_once = true;
            t.seeding = true;
            t.seed_since = Some(now_ms());
            item.status = Status::Done;
            item.finished_at = Some(now_ms());
            item.file_path = Some(if t.files > 1 {
                t.output_dir.clone()
            } else {
                Path::new(&t.output_dir).join(&item.file_name).to_string_lossy().into_owned()
            });
            changed = true;
            completed = true;
        }
        // Seeding limits.
        if item.status == Status::Done && t.seeding {
            let size = item.size.unwrap_or(0).max(1) as f32;
            let ratio_hit = settings.seed_ratio > 0.0 && t.uploaded as f32 / size >= settings.seed_ratio;
            let time_hit =
                settings.seed_minutes > 0 && t.seed_since.is_some_and(|since| now_ms() - since >= settings.seed_minutes as i64 * 60_000);
            if ratio_hit || time_hit {
                t.seeding = false;
                let (h, tor) = (t.info_hash.clone(), self.torrents.clone());
                tokio::spawn(async move {
                    let _ = tor.pause(&h).await;
                });
                return (true, completed);
            }
        }
        let left = item.size.map(|sz| sz.saturating_sub(item.downloaded));
        let downloading = item.status == Status::Downloading;
        if downloading {
            totals.speed += s.down_speed;
            totals.active += 1;
            totals.remaining_bytes += left.unwrap_or(0);
        }
        out.push(Progress {
            id: item.id.clone(),
            downloaded: item.downloaded,
            size: item.size,
            speed: s.down_speed,
            eta: left.filter(|_| s.down_speed > 0 && downloading).map(|l| l / s.down_speed),
            active_connections: s.peers as usize,
            gaps: vec![],
            heads: vec![],
            parts: vec![],
            parts_total: 0,
            file_name: item.file_name.clone(),
            phase: None,
            torrent: Some(TorrentLive {
                up_speed: s.up_speed,
                uploaded: s.uploaded,
                peers: s.peers,
                seeds: s.seeds,
                known_peers: s.known_peers,
                checking: s.state == TState::Checking,
                seeding: !downloading,
                pieces: if visible { self.torrents.piece_map(&t.info_hash, PIECE_BUCKETS).unwrap_or_default() } else { vec![] },
            }),
        });
        (changed, completed)
    }

    /// The torrent view is open: keep the session up (start it if it was stopped, so a paused
    /// torrent's trackers and files can be seen and edited).
    pub(super) fn viewing_torrent(self: &Arc<Self>) {
        self.torrents_viewed_at.store(now_ms(), std::sync::atomic::Ordering::Relaxed);
        if !self.torrents.is_started() {
            let tor = self.torrents.clone();
            tokio::spawn(async move {
                let _ = tor.start().await;
            });
        }
    }

    /// Free the torrent session (memory, sockets, DHT) once it has had nothing to download or
    /// seed for a minute, e.g. after a preview that was cancelled or once every torrent is
    /// paused. It starts again on the next preview, add or resume.
    pub(super) async fn stop_idle_torrents(&self) {
        use std::sync::atomic::Ordering::Relaxed;
        if !self.torrents.is_started() || Inner::has_active_torrents(&self.state.lock()) {
            self.torrents_idle_since.store(0, Relaxed);
            return;
        }
        let now = now_ms();
        if now - self.torrents_viewed_at.load(Relaxed) < torrents_idle_ms() {
            self.torrents_idle_since.store(0, Relaxed);
            return;
        }
        let since = self.torrents_idle_since.load(Relaxed);
        if since == 0 {
            self.torrents_idle_since.store(now, Relaxed);
        } else if now - since >= torrents_idle_ms() && self.torrents.stop_if_idle().await {
            self.torrents_idle_since.store(0, Relaxed);
            tracing::info!("torrent session stopped: nothing to download or seed");
        }
    }

    pub(super) fn has_active_torrents(st: &State) -> bool {
        st.items.values().any(|i| {
            i.torrent.as_ref().is_some_and(|t| i.status == Status::Downloading || (i.status == Status::Done && t.seeding))
        })
    }
}

fn friendly_torrent_text(e: &str) -> String {
    let l = e.to_lowercase();
    match () {
        _ if l.contains("invalid magnet") || l.contains("error parsing magnet") => "That magnet link is not valid".into(),
        _ if l.contains("bencode") || l.contains("deserializ") => "That file is not a valid .torrent".into(),
        _ if l.contains("no space") => "The disk is full".into(),
        _ if l.contains("access is denied") || l.contains("permission") => "Windows refused access to the download folder".into(),
        _ => e.chars().take(160).collect(),
    }
}

fn friendly_torrent_error(e: &anyhow::Error) -> String {
    friendly_torrent_text(&format!("{e:#}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_a_tracker_list() {
        let text = "udp://tracker.example.org:1337/announce\n\n# a comment\nhttps://t.example.net/announce\nudp://tracker.example.org:1337/announce\nnot a tracker\nftp://x.example/announce\n  http://spaced.example/announce  \n";
        assert_eq!(
            super::parse_tracker_list(text),
            ["udp://tracker.example.org:1337/announce", "https://t.example.net/announce", "http://spaced.example/announce"]
        );
        let many: String = (0..150).map(|i| format!("udp://t{i}.example:80/announce\n")).collect();
        assert_eq!(super::parse_tracker_list(&many).len(), 100);
        assert!(super::parse_tracker_list("").is_empty());
    }

    #[test]
    fn spots_torrent_links() {
        assert!(super::is_torrent_link("magnet:?xt=urn:btih:abc"));
        assert!(super::is_torrent_link("https://cdimage.debian.org/x/debian.iso.torrent"));
        assert!(!super::is_torrent_link("https://example.com/file.zip"));
        assert!(!super::is_torrent_link("C:\\nope\\missing.torrent"));
    }
}
