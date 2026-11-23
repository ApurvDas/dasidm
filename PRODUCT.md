# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

DasIDM is a Windows desktop app whose UI is HTML/CSS rendered in WebView2 (Tauri). It ships for **Windows only** (Windows 10/11); the interface may lean on Windows conventions (title bar controls, Mica-era materials, Segoe UI Variable, Windows keyboard shortcuts, Windows power actions) without hedging for macOS or Linux.

## Stack

Delegated, with the user's binding requirement: "the fastest tech stack which will be optimized as much as possible."
- Engine and application core: Rust (tokio, reqwest with rustls, SQLite).
- Desktop shell: Tauri 2 (native WebView2, small binary).
- UI: Svelte 5 + TypeScript + Vite, hand-written CSS with design tokens; no virtual DOM, no heavy UI kit.

## Users

The owner (a power user) first, then friends and the public who install it themselves. Users run it all day in the background on a Windows PC and come to the window to add, watch, sort and open downloads. Newcomers need first-run guidance and plain labels; the owner needs density and speed.

## Product Purpose

A personal Internet Download Manager: download anything from the internet faster and more reliably than a browser, keep it organised, and never lose progress. Success means downloads finish at full line speed, survive pauses, crashes and network drops, and land in the right folder without the user thinking about it.

## Positioning

The user's own IDM, written from scratch: one app that handles plain files over many parallel connections, video and music from websites (including lossless FLAC), and torrents, with browser capture, in a native-fast Rust core instead of an Electron or Java shell.

## Operating Context

- Lives in the Windows tray; the main window is opened on demand.
- Downloads arrive from the browser extension, pasted URLs, the clipboard monitor, batch lists, site grabs, magnets and .torrent files.
- Download mix, all confirmed as primary: big files (games, ISOs, installers, archives), videos and music (MP4/MP3 and FLAC), torrents, and large batches of small files.
- Users check progress at a glance, open or reveal finished files, and manage queues and schedules (e.g. overnight, shut down when done).

## Capabilities and Constraints

Phase 1 (core):
- Segmented multi-connection HTTP/HTTPS engine with dynamic work stealing, pause/resume with server-change validation, retries with backoff, speed limits, crash-safe state in SQLite.
- Queue with concurrency limit, categories by file type with per-category folders, settings.

Later phases (all confirmed wanted):
- Browser capture extension (Chrome/Edge first).
- Video and music from websites via yt-dlp + ffmpeg: MP4 quality picking, MP3 and FLAC audio.
- BitTorrent and magnet links.
- Scheduler, "shut down when done", clipboard monitor, batch downloads, site grabber.

Binding performance constraint from the user: "minimum ram usage on the idle app".
- Closing or minimising to the tray destroys the WebView window; only the Rust core keeps running. The window is recreated on demand.
- Glass comes from OS-level Mica/Acrylic, not CSS `backdrop-filter` layers, which cost GPU memory.
- The UI receives progress only while visible, the list is virtualised, and no heavy UI framework runtime is shipped.

Undecided: translations (English first; others possible later since the app goes public), installer and update channel.

## Brand Commitments

- Name: **DasIDM**.
- Binding visual commitment from the user: "i wat a glass like design". Glass is the window material: Windows 11 Mica/Acrylic supplied by the OS compositor (Tauri window effects), with translucent panels over it. Whatever visual world is chosen, its material renders as glass.
- No logo, mascot, or voice guidelines exist yet.

## Evidence on Hand

None. There are no benchmarks, testimonials, user counts or press. Never fabricate speed claims ("15× faster"), download counts, or reviews; any performance figure must come from a real measurement of DasIDM.

## Product Principles

1. **Speed is the product.** Every screen and every millisecond of UI work must stay out of the engine's way; the UI is light enough to keep open beside thousands of items.
2. **Never lose progress.** State is durable; pausing, closing, crashing or losing the network is always recoverable, and the UI says plainly what will happen.
3. **Mixed media is first-class.** Files, videos, music and torrents share one list and one mental model, while each still shows what matters for its kind (connections, quality, peers).
4. **Power without a manual.** Dense and keyboard-driven for the owner, but self-explanatory for a friend installing it for the first time.

## Accessibility & Inclusion

No specific requirement was stated. Because the app is going public, meet WCAG 2.2 AA contrast and full keyboard operation as the baseline.
