---
version: 1
slug: "app-src-app-svelte"
primary_target: "app/src/App.svelte"
related_targets: ["app/src/components/PartsView.svelte"]
---

# Main window

Scope: DasIDM main window (Windows desktop app, WebView2). Visitor mode: **Operate**.

Audience and job: the owner and friends who installed it; open the window from the tray to add a URL, see what is loading and when it will finish, see the total bandwidth used, find and open finished files, fix failures, and see how each file is split across connections. Used several times a day, mouse and keyboard equally (Ctrl+V adds, Space pauses/resumes, P opens parts, Delete removes, Ctrl+F searches).

States that must be designed: loading (multi-connection), queued, paused, failed with a fix action, completed, empty list, empty filter result, no network, parts splitting (work stealing). "Scheduled" is deferred to the scheduler phase (no scheduler exists yet; the user confirmed it as a later phase).

Constraints: glass material is binding (Windows 11 Mica from the OS; translucent panels over it, no CSS backdrop blur). Minimum RAM at idle is binding: the window is destroyed when hidden, so first paint must be fast and the UI must hold thousands of rows with a virtualised list. Parts animation uses anime.js (user's request) and honours reduced motion.

Phase-1 deviations from the first-viewport sketch, accepted: title-bar tabs are status filters (All, Loading, Waiting, Paused, Finished, Failed) because torrents and queues are later phases; the status bar shows speed limit, lines, downloads at once and the save folder, with free space and browser capture to join when browser capture ships.

## Direction contract

THESIS: DasIDM reads like a departures board: what is loading is ranked by when it will finish, and anything that changes stays lit until you've seen it (or fades after a few seconds). It refuses the category default of a flat table with thin blue progress bars where every row looks the same.

OWN-WORLD: smoked-glass board panels (ink #0B0D10 / slate #1C2127 at 55–75% over Mica), fog #9AA1A9 secondary, panel ivory #F2F2EE text and selection, one alert yellow #FFD400 reserved for change, live write heads and the primary action, green done, red failed. Board data in condensed Martian Mono with tabular figures, uppercase tracked labels, big condensed gothic numerals (Barlow Condensed) for times, totals and part numbers; body in Segoe UI Variable. Segmented ivory "pass" with dashed tear lines; the scannable block is the file's live parts map.

STORY: at a glance you know what is loading, the total speed, when everything finishes, and what just changed; you trust that the order means something; you act from the row or the pass; one key shows how the file is split and how the engine keeps every line busy.

FIRST VIEWPORT: glass title bar with DASIDM wordmark, status-filter tabs, Pause all, settings and yellow "+ Add URL", then window controls. Top: the departures board of active items ranked by finish time (finish time, file, size, speed, status) with total speed, its 60-second trace and "all done at" in the header. Below: the full list as the quieter hall (sortable, virtualised). Right: the pass for the selected item (from, to, loaded/total, done at, speed, lines, resume) with the parts map and "Open parts". Parts view (P) replaces board and hall: header totals, a numbered file map, one row per live connection (part number, byte range, progress, done %, speed).

FORM: vernacular-ephemera-boarding-pass-and-gate-board (dealt challenger, competitive, chosen by the user over the assigned Harbour Manifest, position 7 of 7 on my grounded list). seed:6f306abb1cc17ea0. Signature interactions: (1) a board row reranks in place with an animated move, only on a real overtake, and holds its yellow until seen or 8 s; reranking freezes under the pointer. (2) Parts split: a fast line takes half of a slow part; the map drops a yellow cut and flash, the new row opens in file order reading "SPLIT FROM #NN", finished rows close up (anime.js). Kept disciplines: tabular no-jitter figures; real media art is the only imagery; state as a mark in a fixed cell; one type size inside the list.

FINISH: unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance
