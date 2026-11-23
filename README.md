# DasIDM

A personal download manager for Windows, written from scratch in Rust (engine + core), Tauri 2 (shell) and Svelte 5 (UI).

## What works (phase 1)

- **Multi-connection engine** (`crates/das-engine`): 1 to 32 parallel connections per file with dynamic work stealing (an idle connection takes half of the biggest remaining range, so every line stays busy until the last byte). Data is written straight to its final offset in `name.ext.dasdl` (sparse on NTFS) and renamed when done.
- **Pause / resume / crash-safe:** resume points are checkpointed every 2 s (fsync first, then the database), and validated against the server's ETag / Last-Modified on resume.
- Retries with exponential backoff; drops to fewer connections when a server refuses extra ones; servers without range support fall back to one connection.
- **Speed limit** (global, live), **queue** with "N at once", **categories** by file type with optional per-type folders.
- **UI:** "Gate Board" design on Windows 11 Mica glass: a departures board of active downloads ranked by finish time, a virtualised list of everything, and a boarding-pass detail with a live map of every connection.
- **Low idle RAM:** closing the window destroys the WebView entirely; downloads continue in the tray with only the Rust core in memory.

- **Parts view:** every download's pass shows a live map of its parts; **Open parts** (`P`) shows each connection as its own row with byte range, progress and speed. When a fast line takes over half of a slow line's part, the cut flashes yellow and the new row reads "SPLIT FROM #NN" (animated with anime.js; off under reduced motion).

## What works (v0.1.1)

- **Video and music links** (YouTube and the other sites yt-dlp reads): Add URL shows a **quality picker** instead of saving the web page. Video: every resolution the site has, as "MP4 · H.264 · plays everywhere" or "best quality" (VP9/AV1, WebM/MKV), with sizes. Sound only: M4A (original, not re-encoded), MP3 320, Opus (original), FLAC (labelled honestly: the source is lossy). Playlists: "This video" or "Whole playlist". Pause, resume, retry on YouTube's occasional 403, and remove all work; titles, artist and (for music) cover art are embedded.
- **Video tools on demand:** the first video link asks once to fetch yt-dlp, ffmpeg and deno (about 158 MB; about 320 MB unpacked) from the publishers' GitHub releases into `%LOCALAPPDATA%\DasIDM\tools`. Every file is checked against the publisher's SHA-256 list before it is used; yt-dlp is refreshed weekly. Each job works in its own hidden `.dasidm` folder and the finished file is moved next to yours under a free name, so a video job never overwrites a file.
- **Never saves a web page by accident:** a link that answers with HTML shows "This link is a web page, not a file" with a deliberate **Save page anyway**.
- **Add URL picks up the clipboard:** `Add URL` / `Ctrl+N` open prefilled with the first link on the clipboard ("From clipboard").
- **Browse…** opens the Windows folder picker for the Add sheet's folder and Settings' download folder.
- **File types told apart:** an icon per type (video, music, program, archive, disc image, document, image, torrent, web page, other), a sortable Type column, type chips with counts above the list, and for media the quality and length (`1080p · 21:08`, `MP3 · 3:58`), channel and real thumbnail on the pass. Types come from the extension, or the server's Content-Type when the name has none.

## What works (v0.2.0): torrents

- **Magnet links, .torrent links and .torrent files** (Add URL, `Ctrl+V`, **Open .torrent…**, drag a file onto the window, or `dasidm.exe <magnet or file>`). DasIDM reads the file list first; choose which files to fetch and add extra trackers. Many-file torrents get their own folder.
- **qBittorrent-style editable trackers** while a torrent runs: each tracker's status (Working, Not working with the reason, Updating, Not contacted, Disabled), tier, seeds, peers, peers received and next announce; add, edit, remove, **Ask all now**, copy all. Edits survive restarts. This needed a small patch to librqbit (see `third_party/NOTICE.md`).
- **Files** can be included or skipped while the torrent runs; a **piece map** shows what is verified.
- **Seeding** after the download, until a ratio or time limit (Settings); **Stop seeding** / **Seed** on finished torrents. Upload limit, a default tracker list for every new torrent, a fixed incoming port with UPnP, DHT on or off, and "Use DasIDM for magnets and .torrent files" (this Windows user only).
- **Low idle RAM still holds:** the BitTorrent session starts only while a torrent is downloading or seeding.

Keyboard: `Ctrl+V` add a copied link · `Ctrl+N` add · `Space` pause/resume · `Enter` open · `P` parts view · `Del` remove · `Ctrl+F` search · `↑ ↓` move.

Add from the command line (also hands links to a running DasIDM): `dasidm.exe https://example.com/file.iso`. A video link given this way opens the quality picker.

## Coming next (agreed phases)

Browser capture extension, faster video via DasIDM's own multi-connection engine, scheduler and "shut down when done", clipboard monitor, batch downloads, site grabber.

## Build and run

Requirements: Rust (MSVC), Node 20+, WebView2 (built into Windows 11).

```powershell
cd app
npm install
npx tauri dev                 # run with hot reload
npx tauri build               # installer in target/release/bundle/nsis
npx tauri build --no-bundle   # just target/release/dasidm.exe
```

`npm run dev` alone serves the UI in a normal browser with synthetic preview data (no Rust needed) for design work.

Tests:

```powershell
cargo test -p das-engine -p das-core
```

## Git workflow

- `main` is always releasable; each release is a tag (`v0.1.0`, …). Remote: private GitHub repo `ApurvDas/dasidm`.
- New work goes on a branch named for the feature (`feature/media-and-add-flow`), one commit per milestone with tests passing, then a pull request into `main`.
- Commit messages follow Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `test:`).
- Plans for upcoming phases live in `docs/plans/` (e.g. `docs/plans/torrents.md`).

## Layout

```
crates/das-engine   download engine (no UI, no DB)
crates/das-core     manager: SQLite, queue, categories, settings, progress events
app/src-tauri       Tauri shell: commands, tray, Mica window, low-RAM lifecycle
app/src             Svelte UI
```

Data lives in `%APPDATA%\dev.dasidm.app\dasidm.db`. Start minimised to the tray with `dasidm.exe --background`.
