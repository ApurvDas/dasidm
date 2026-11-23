// rqbit-patch (DasIDM): a tracker list that can change while a torrent runs, with a live
// status per tracker (like qBittorrent's Trackers tab).

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use parking_lot::Mutex;
use tokio::sync::watch;
use url::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackerState {
    /// Not asked yet.
    NotContacted,
    /// A request is on its way.
    Updating,
    /// The last announce worked.
    Working,
    /// The last announce failed; `message` says why. It is retried on its own.
    NotWorking,
    /// Never contacted (unsupported scheme, or trackers are off for this torrent).
    Disabled,
}

#[derive(Clone, Debug)]
pub struct TrackerStatus {
    pub url: Url,
    /// Lower tiers are tried first by most clients; all tiers are announced to here.
    pub tier: u32,
    pub state: TrackerState,
    pub seeders: Option<u64>,
    pub leechers: Option<u64>,
    /// Peers this tracker has handed out since the torrent started.
    pub peers_received: u64,
    pub last_announce: Option<SystemTime>,
    pub next_announce: Option<SystemTime>,
    pub message: Option<String>,
}

impl TrackerStatus {
    fn new(url: Url, tier: u32) -> Self {
        let supported = matches!(url.scheme(), "http" | "https" | "udp");
        Self {
            state: if supported { TrackerState::NotContacted } else { TrackerState::Disabled },
            message: (!supported).then(|| format!("{}:// trackers are not supported", url.scheme())),
            url,
            tier,
            seeders: None,
            leechers: None,
            peers_received: 0,
            last_announce: None,
            next_announce: None,
        }
    }
}

/// The trackers of one torrent. Cheap to share; every change wakes the announce loops.
pub struct TrackerSet {
    list: Mutex<Vec<TrackerStatus>>,
    changes: watch::Sender<u64>,
    reannounce: watch::Sender<u64>,
}

impl std::fmt::Debug for TrackerSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.list.lock().iter().map(|t| t.url.as_str())).finish()
    }
}

impl TrackerSet {
    /// Trackers in announce order; each gets the tier it is given.
    pub fn new(trackers: impl IntoIterator<Item = (Url, u32)>) -> Arc<Self> {
        let mut list: Vec<TrackerStatus> = Vec::new();
        for (url, tier) in trackers {
            if !list.iter().any(|t| t.url == url) {
                list.push(TrackerStatus::new(url, tier));
            }
        }
        Arc::new(Self { list: Mutex::new(list), changes: watch::channel(0).0, reannounce: watch::channel(0).0 })
    }

    pub fn list(&self) -> Vec<TrackerStatus> {
        self.list.lock().clone()
    }

    pub fn urls(&self) -> Vec<Url> {
        self.list.lock().iter().map(|t| t.url.clone()).collect()
    }

    fn changed(&self) {
        self.changes.send_modify(|n| *n += 1);
    }

    /// Add trackers that are not there yet, in a new tier after the existing ones (or `tier`).
    /// Returns how many were added.
    pub fn add(&self, urls: impl IntoIterator<Item = Url>, tier: Option<u32>) -> usize {
        let added = {
            let mut list = self.list.lock();
            let tier = tier.unwrap_or_else(|| list.iter().map(|t| t.tier + 1).max().unwrap_or(0));
            let mut n = 0;
            for url in urls {
                if !list.iter().any(|t| t.url == url) {
                    list.push(TrackerStatus::new(url, tier));
                    n += 1;
                }
            }
            n
        };
        if added > 0 {
            self.changed();
        }
        added
    }

    /// Stop announcing to these trackers. Returns how many were removed.
    pub fn remove(&self, urls: &[Url]) -> usize {
        let removed = {
            let mut list = self.list.lock();
            let before = list.len();
            list.retain(|t| !urls.contains(&t.url));
            before - list.len()
        };
        if removed > 0 {
            self.changed();
        }
        removed
    }

    /// Edit one tracker's address, keeping its place and tier. False if `old` is unknown or
    /// `new` is already listed.
    pub fn replace(&self, old: &Url, new: Url) -> bool {
        let ok = {
            let mut list = self.list.lock();
            if list.iter().any(|t| t.url == new) {
                false
            } else if let Some(t) = list.iter_mut().find(|t| &t.url == old) {
                *t = TrackerStatus::new(new, t.tier);
                true
            } else {
                false
            }
        };
        if ok {
            self.changed();
        }
        ok
    }

    /// Ask every tracker again now instead of waiting for its interval.
    pub fn reannounce(&self) {
        self.reannounce.send_modify(|n| *n += 1);
    }

    /// Mark every tracker as off (used when trackers are disabled for the session).
    pub fn disable_all(&self, why: &str) {
        for t in self.list.lock().iter_mut() {
            t.state = TrackerState::Disabled;
            t.message = Some(why.to_string());
        }
    }

    pub(crate) fn update(&self, url: &Url, f: impl FnOnce(&mut TrackerStatus)) {
        if let Some(t) = self.list.lock().iter_mut().find(|t| &t.url == url) {
            f(t);
        }
    }

    pub(crate) fn set_next(&self, url: &Url, after: Duration) {
        self.update(url, |t| t.next_announce = SystemTime::now().checked_add(after));
    }

    pub(crate) fn subscribe_changes(&self) -> watch::Receiver<u64> {
        self.changes.subscribe()
    }

    pub(crate) fn subscribe_reannounce(&self) -> watch::Receiver<u64> {
        self.reannounce.subscribe()
    }
}

/// A short reason for the Trackers list (never the full request URL with its query).
pub(crate) fn short_error(e: &anyhow::Error) -> String {
    if let Some(r) = e.downcast_ref::<reqwest::Error>() {
        if r.is_timeout() {
            return "Timed out".into();
        }
        if r.is_connect() {
            return "Can't connect".into();
        }
        if let Some(s) = r.status() {
            return format!("HTTP {}", s.as_u16());
        }
        if r.is_decode() || r.is_body() {
            return "Bad answer from the tracker".into();
        }
        return "Request failed".into();
    }
    let s = format!("{e:#}");
    if let Some(reason) = s.split("Failure reason: ").nth(1) {
        return format!("Tracker says: {}", reason.chars().take(140).collect::<String>());
    }
    if s.contains("timed out") || s.contains("timeout") {
        return "Timed out".into();
    }
    if s.contains("looking up hostname") || s.contains("zero addresses") {
        return "Can't find the tracker's address".into();
    }
    s.chars().take(140).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn edits_the_list() {
        let set = TrackerSet::new([(u("udp://a.example:80/announce"), 0), (u("http://b.example/announce"), 0)]);
        let mut changes = set.subscribe_changes();
        assert_eq!(set.add([u("http://b.example/announce"), u("https://c.example/announce")], None), 1);
        assert!(changes.has_changed().unwrap());
        changes.mark_unchanged();
        assert_eq!(set.list()[2].tier, 1);
        assert!(set.replace(&u("http://b.example/announce"), u("http://b2.example/announce")));
        assert!(!set.replace(&u("http://nope.example/"), u("http://x.example/")));
        assert_eq!(set.remove(&[u("udp://a.example:80/announce")]), 1);
        assert_eq!(
            set.urls().iter().map(Url::as_str).collect::<Vec<_>>(),
            ["http://b2.example/announce", "https://c.example/announce"]
        );
        assert!(changes.has_changed().unwrap());
    }

    #[test]
    fn unsupported_schemes_are_disabled() {
        let set = TrackerSet::new([(u("wss://tracker.example/"), 0)]);
        let t = &set.list()[0];
        assert_eq!(t.state, TrackerState::Disabled);
        assert!(t.message.as_deref().unwrap().contains("wss"));
    }
}
