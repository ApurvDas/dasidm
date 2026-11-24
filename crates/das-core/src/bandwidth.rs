//! One speed limit for every kind of download, and torrent uploads that don't slow downloads.
//!
//! A full upload link chokes downloads (the acknowledgements a download needs queue behind the
//! upload), so while anything downloads DasIDM holds torrent uploads to 80% of the fastest
//! upload it has seen. That speed is learned only while uploads run uncapped (seeding, or before
//! the cap is known), so a slow first reading can't lock the cap low.

use super::*;
use serde::{Deserialize, Serialize};

/// What DasIDM has learned about the line's upload speed; kept in `upload.json`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UploadLearned {
    /// Fastest total torrent upload seen while uploads were uncapped, bytes per second.
    pub peak_bps: u64,
    /// Seconds of real uploading watched so far.
    pub seconds: u32,
}

/// For Settings: the cap in force now and what it's based on.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadStatus {
    pub peak_bps: u64,
    pub learned_seconds: u32,
    pub learn_seconds: u32,
    /// The torrent upload limit applied now (0 = none).
    pub cap_bps: u64,
}

/// Seconds of uploading to watch before the cap is trusted.
pub const LEARN_SECS: u32 = 60;
/// Never hold uploads below this; peers stop trading with a client that gives nothing.
const MIN_CAP: u64 = 32 * 1024;
/// Only seconds with real upload traffic teach anything.
const MIN_UPLOAD_TO_COUNT: u64 = 16 * 1024;

#[derive(Default)]
pub(super) struct Bandwidth {
    learned: UploadLearned,
    learn_ms: u64,
    saved: UploadLearned,
    /// (HTTP download, torrent download, torrent upload) limits last applied.
    applied: Option<(u64, u64, u64)>,
    last_ms: i64,
}

impl Bandwidth {
    pub(super) fn load(data_dir: &Path) -> Self {
        let learned: UploadLearned =
            std::fs::read(data_dir.join("upload.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        Self { learned, learn_ms: learned.seconds as u64 * 1000, saved: learned, ..Default::default() }
    }

    /// Forget what was applied, so the next balance sets every limit again (after Settings change).
    pub(super) fn reapply(&mut self) {
        self.applied = None;
    }
}

/// How the speed limit is shared: half each while both HTTP and torrents download, otherwise
/// all of it to whichever does. Returns (HTTP, torrents); 0 = no limit.
pub fn split_limit(limit: u64, http_active: bool, torrents_downloading: bool) -> (u64, u64) {
    if limit == 0 {
        return (0, 0);
    }
    if http_active && torrents_downloading { (limit / 2, limit - limit / 2) } else { (limit, limit) }
}

/// The torrent upload limit to apply: the user's own limit if set; otherwise, while anything
/// downloads and the line's upload speed is known, 80% of it.
pub fn upload_cap(user_limit: u64, protect: bool, downloading: bool, learned: UploadLearned) -> u64 {
    if user_limit > 0 {
        return user_limit;
    }
    if protect && downloading && learned.seconds >= LEARN_SECS && learned.peak_bps > 0 {
        (learned.peak_bps / 5 * 4).max(MIN_CAP)
    } else {
        0
    }
}

impl Inner {
    /// Every 2 s: share the speed limit between HTTP and torrents, and set the upload cap.
    pub(super) fn balance_bandwidth(&self) {
        let s = self.settings.read().clone();
        let (http_active, torrent_hashes, torrents_downloading) = {
            let st = self.state.lock();
            let hashes: Vec<String> = st
                .items
                .values()
                .filter_map(|i| {
                    let t = i.torrent.as_ref()?;
                    (i.status == Status::Downloading || (i.status == Status::Done && t.seeding)).then(|| t.info_hash.clone())
                })
                .collect();
            let downloading = st.items.values().any(|i| i.kind == Kind::Torrent && i.status == Status::Downloading);
            (!st.running.is_empty(), hashes, downloading)
        };
        let up_now: u64 = if self.torrents.is_started() {
            torrent_hashes.iter().filter_map(|h| self.torrents.stats(h)).map(|x| x.up_speed).sum()
        } else {
            0
        };

        let mut bw = self.bandwidth.lock();
        let now = now_ms();
        let dt = if bw.last_ms == 0 { 0 } else { (now - bw.last_ms).clamp(0, 10_000) as u64 };
        bw.last_ms = now;
        // Learn only from uncapped uploading.
        let uncapped = bw.applied.is_none_or(|a| a.2 == 0);
        if uncapped && up_now >= MIN_UPLOAD_TO_COUNT {
            bw.learned.peak_bps = bw.learned.peak_bps.max(up_now);
            bw.learn_ms += dt;
            bw.learned.seconds = (bw.learn_ms / 1000).min(u32::MAX as u64) as u32;
        }

        let up = upload_cap(s.upload_limit, s.protect_downloads, http_active || torrents_downloading, bw.learned);
        let (http, torrent) = split_limit(s.speed_limit, http_active, torrents_downloading);
        if bw.applied != Some((http, torrent, up)) {
            self.engine.global_limiter().set_rate(http);
            self.torrents.set_limits(torrent, up);
            bw.applied = Some((http, torrent, up));
        }

        let (l, saved) = (bw.learned, bw.saved);
        let worth_saving = l != saved
            && (l.peak_bps > saved.peak_bps + saved.peak_bps / 20
                || (saved.seconds < LEARN_SECS && l.seconds >= LEARN_SECS)
                || l.seconds >= saved.seconds + 60);
        if worth_saving {
            bw.saved = l;
            drop(bw);
            if let Ok(json) = serde_json::to_vec(&l) {
                let tmp = self.data_dir.join("upload.json.tmp");
                if std::fs::write(&tmp, json).is_ok() {
                    let _ = std::fs::rename(tmp, self.data_dir.join("upload.json"));
                }
            }
        }
    }

    pub(super) fn upload_status(&self) -> UploadStatus {
        let bw = self.bandwidth.lock();
        UploadStatus {
            peak_bps: bw.learned.peak_bps,
            learned_seconds: bw.learned.seconds,
            learn_seconds: LEARN_SECS,
            cap_bps: bw.applied.map(|a| a.2).unwrap_or(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limit_is_shared_only_while_both_download() {
        assert_eq!(split_limit(0, true, true), (0, 0));
        assert_eq!(split_limit(1000, true, true), (500, 500));
        assert_eq!(split_limit(1001, true, false), (1001, 1001));
        assert_eq!(split_limit(1000, false, true), (1000, 1000));
    }

    #[test]
    fn upload_cap_waits_for_a_known_speed() {
        let known = UploadLearned { peak_bps: 1_000_000, seconds: LEARN_SECS };
        let early = UploadLearned { peak_bps: 1_000_000, seconds: LEARN_SECS - 1 };
        // The user's own limit always wins.
        assert_eq!(upload_cap(50_000, true, false, known), 50_000);
        // 80% while downloading, once learned.
        assert_eq!(upload_cap(0, true, true, known), 800_000);
        // Nothing downloading, still learning, or switched off: no cap.
        assert_eq!(upload_cap(0, true, false, known), 0);
        assert_eq!(upload_cap(0, true, true, early), 0);
        assert_eq!(upload_cap(0, false, true, known), 0);
        // A slow line still uploads something.
        assert_eq!(upload_cap(0, true, true, UploadLearned { peak_bps: 20_000, seconds: 600 }), MIN_CAP);
    }
}
