# Phase 2a: torrents with qBittorrent-style editable trackers (built in v0.2.0)

**Research (librqbit 9.1.0, Apache-2.0):** it has magnets, .torrent files and URLs, DHT, file selection (`only_files`, plus `update_only_files` live), pause/unpause/delete, fast-resume with JSON persistence, and live session limits (`Session.ratelimits.set_*_bps`). It has **no runtime tracker API** (`ManagedTorrentShared.trackers` is a frozen `HashSet<Url>`) and **no per-tracker status**. Decision: **patch the library**.

1. **Spike gate:** librqbit with `default-features=false, features=["sha1-crypto-hash"]` (wincrypt SHA-1, HTTPS via our reqwest native-tls). Confirm the MSVC build. Download an official Debian/Arch magnet. Measure size and RAM. Fallback: `rust-tls`.
2. **Vendor + patch** `third_party/rqbit` at tag v9.1.0 (git subtree, `[patch.crates-io]` for `librqbit` and `librqbit-tracker-comms`, `rqbit-patch:` commits):
   - mutable tracker list with a change channel;
   - one announce loop per tracker, started and stopped live;
   - `TrackerStatus {url, tier, state NotContacted|Updating|Working|NotWorking|Disabled, seeders, leechers, peers_received, last/next announce, message}`;
   - API `trackers()`, `add_trackers`, `remove_trackers`, `replace_tracker`, `reannounce()`;
   - persisted edits; `have_pieces()`; patch tests.
3. **`crates/das-torrent`:**
   - a lazy session (started only when torrents exist, to keep idle RAM low), with JSON persistence and fastresume in `<data>/torrents`, DHT, port and UPnP from settings;
   - global limits wired to DasIDM's speed limit;
   - `preview` (list_only file tree), `add`, `pause`, `resume`, `remove`, `set_files`, `stats` (↓/↑, peers/seeds, ratio, ETA, metadata/seeding/stalled), tracker operations, `pieces`;
   - default trackers appended at add time, plus "add to all existing";
   - seeding stops at a ratio or time.
4. **Core:**
   - `Item.kind` gains `Torrent`, routed through the `manager.rs` schedule, finished and progress paths;
   - add sources: magnet links and .torrent URLs in Add URL, an Open .torrent dialog, drag-and-drop, the CLI or second instance;
   - a BitTorrent settings section;
   - an optional per-user HKCU association for magnet links and .torrent files.
5. **UI** (Gate Board extension):
   - an add preview with a file tree and an extra-trackers box;
   - torrent row statuses and a seeding mark;
   - the pass shows `SWARM → folder`, seeds/peers, ↓/↑, ratio and a piece map;
   - a Torrent view with tabs:
     - **Trackers:** URL, tier, status, seeds, peers, leechers and message; add, edit and remove live, reannounce, copy all; DHT/PEX rows. A failing tracker's row lights up.
     - **Files:** live include/skip.
     - **Pieces:** a large piece map.
   - a default-trackers textarea in Settings; the impeccable review.
6. **Tests:** a local seeder and downloader session pair plus a tiny axum tracker. They check the file arrives byte-exact, status reaches Working, a tracker added live gets announced, a removed one stops, a dead URL reads Not working, and resume works after a restart.
7. **Packaging:** move to the NSIS per-user installer. Windows Firewall will ask once to allow incoming connections for seeding.
