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

Keyboard: `Ctrl+V` add a copied link · `Ctrl+N` add · `Space` pause/resume · `Enter` open · `P` parts view · `Del` remove · `Ctrl+F` search · `↑ ↓` move.

Add from the command line (also hands links to a running DasIDM): `dasidm.exe https://example.com/file.iso`.

## Coming next (agreed phases)

Browser capture extension, video/music grabbing (yt-dlp + ffmpeg, MP4 / MP3 / FLAC), torrents and magnets, scheduler and "shut down when done", clipboard monitor, batch downloads, site grabber.

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

## Layout

```
crates/das-engine   download engine (no UI, no DB)
crates/das-core     manager: SQLite, queue, categories, settings, progress events
app/src-tauri       Tauri shell: commands, tray, Mica window, low-RAM lifecycle
app/src             Svelte UI
```

Data lives in `%APPDATA%\dev.dasidm.app\dasidm.db`. Start minimised to the tray with `dasidm.exe --background`.
