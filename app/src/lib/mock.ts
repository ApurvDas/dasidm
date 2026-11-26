// SYNTHETIC preview data: only loaded when the UI runs in a plain browser (no Tauri),
// so the design can be reviewed without the Rust core. Never shipped as real content.
import type { AddRequest, AddTorrentRequest, AddVideoRequest, CoreEvent, ItemView, Progress, Settings, ToolsStatus, TorrentPreview, TrackerView, VideoInfo } from './types';

/** ?boost=N speeds the simulation up so a review capture can catch a split. */
const BOOST = Number(new URLSearchParams(location.search).get('boost')) || 1;
const GB = 1024 ** 3;
const MB = 1024 ** 2;
const now = Date.now();
let listeners: ((e: CoreEvent) => void)[] = [];

interface Seg {
  start: number;
  pos: number;
  end: number;
  speed: number;
}
interface Sim {
  item: ItemView;
  speed: number;
  segs: Seg[];
  total: number;
}

function item(p: Partial<ItemView> & Pick<ItemView, 'id' | 'fileName' | 'status' | 'category'>): ItemView {
  return {
    kind: 'http',
    media: null,
    torrent: null,
    url: `https://cdn.example.net/${encodeURIComponent(p.fileName)}`,
    saveDir: `C:\\Users\\you\\Downloads\\${p.category}`,
    filePath: null,
    size: null,
    downloaded: 0,
    addedAt: now - 3600_000,
    finishedAt: null,
    error: null,
    connections: 16,
    resumable: true,
    ...p,
  };
}

// Parts at uneven progress, so some finish soon and steal from the others (shows splits).
function segsFor(size: number, n: number): Seg[] {
  const segs: Seg[] = [];
  const chunk = Math.floor(size / n);
  for (let i = 0; i < n; i++) {
    const start = i * chunk;
    const end = i === n - 1 ? size : start + chunk;
    const done = i % 3 === 0 ? 0.9 + Math.random() * 0.08 : 0.25 + Math.random() * 0.5;
    segs.push({ start, pos: Math.floor(start + (end - start) * done), end, speed: 0 });
  }
  return segs;
}

const sims: Sim[] = [];
const items: ItemView[] = [];

function addSim(it: ItemView, speed: number, lines: number) {
  items.push(it);
  if (it.status === 'downloading') {
    const segs = segsFor(it.size!, lines);
    it.downloaded = it.size! - segs.reduce((a, s) => a + (s.end - s.pos), 0);
    sims.push({ item: it, speed, segs, total: lines });
  }
}

addSim(item({ id: 'a1', fileName: 'Cyberpunk_2077_v2.3_setup.exe', status: 'downloading', category: 'programs', size: 64.8 * GB, downloaded: 26.6 * GB, url: 'https://cdn.gog.com/setup/Cyberpunk_2077_v2.3_setup.exe' }), 22.1 * MB, 16);
addSim(item({ id: 'a2', fileName: 'ubuntu-26.04-desktop-amd64.iso', status: 'downloading', category: 'discs', size: 6.1 * GB, downloaded: 4.75 * GB, connections: 8, url: 'https://releases.ubuntu.com/26.04/ubuntu-26.04-desktop-amd64.iso' }), 9.6 * MB, 8);
addSim(item({ id: 'a3', fileName: 'Harbour Lights - Night Shift (FLAC 24-96).zip', status: 'downloading', category: 'archives', size: 1.9 * GB, downloaded: 1.2 * GB, connections: 4 }), 4.2 * MB, 4);
addSim(item({ id: 'a4', fileName: 'blender-5.0-windows-x64.msi', status: 'downloading', category: 'programs', size: 412 * MB, downloaded: 37 * MB, connections: 2 }), 2.5 * MB, 2);
items.push(item({ id: 'b1', fileName: 'Sprite Fright 4K.mp4', status: 'paused', category: 'video', size: 2.3 * GB, downloaded: 1.26 * GB, addedAt: now - 7200_000 }));
items.push(item({ id: 'b2', fileName: 'archlinux-2026.11.01-x86_64.iso', status: 'queued', category: 'discs', size: 1.4 * GB, addedAt: now - 9000_000 }));
items.push(item({ id: 'b3', fileName: 'photos-2026-backup.zip', status: 'failed', category: 'archives', size: 18.2 * GB, downloaded: 2.2 * GB, error: 'Link expired or access denied', addedAt: now - 12000_000 }));
const done: [string, string, number, number][] = [
  ['VSCodeUserSetup-x64-1.106.exe', 'programs', 118 * MB, 1],
  ['report-q3-final.pdf', 'documents', 4.2 * MB, 2],
  ['Harbour Lights - Day Shift.flac', 'music', 612 * MB, 3],
  ['node-v26.13.0-x64.msi', 'programs', 31.6 * MB, 20],
  ['Steam-installer.exe', 'programs', 2.3 * MB, 26],
  ['wallpaper-pack-2026.zip', 'archives', 288 * MB, 30],
  ['lecture-07-signals.mp4', 'video', 1.1 * GB, 50],
  ['dataset-sample.csv', 'documents', 18 * MB, 70],
];
done.forEach(([name, cat, size, hoursAgo], i) =>
  items.push(
    item({
      id: `d${i}`,
      fileName: name,
      status: 'done',
      category: cat,
      size,
      downloaded: size,
      addedAt: now - hoursAgo * 3600_000 - 60_000,
      finishedAt: now - hoursAgo * 3600_000,
      filePath: `C:\\Users\\you\\Downloads\\${name}`,
    }),
  ),
);

// A finished video and a song from a video site, so media rows and the pass can be reviewed.
items.push(
  item({
    id: 'v1', fileName: 'Building a tiny synth from scratch.mp4', status: 'done', category: 'video', size: 486 * MB, downloaded: 486 * MB,
    kind: 'video', url: 'https://video.example.net/watch?v=demo1', addedAt: now - 5 * 3600_000, finishedAt: now - 5 * 3600_000 + 90_000,
    filePath: 'C:\\Users\\you\\Downloads\\Video\\Building a tiny synth from scratch.mp4',
    media: { title: 'Building a tiny synth from scratch', quality: '1080p', format: 'MP4', audioOnly: false, duration: 1268, channel: 'Bench Notes', site: 'Example', thumb: true },
  }),
  item({
    id: 'v2', fileName: 'Harbour Lights - Low Tide (live).mp3', status: 'done', category: 'music', size: 9.4 * MB, downloaded: 9.4 * MB,
    kind: 'video', url: 'https://video.example.net/watch?v=demo2', addedAt: now - 6 * 3600_000, finishedAt: now - 6 * 3600_000 + 30_000,
    filePath: 'C:\\Users\\you\\Downloads\\Music\\Harbour Lights - Low Tide (live).mp3',
    media: { title: 'Harbour Lights - Low Tide (live)', quality: 'MP3', format: 'MP3', audioOnly: true, duration: 238, channel: 'Harbour Lights', site: 'Example', thumb: true },
  }),
);

// Torrents: a film loading from a swarm, and an album finished and seeding.
const MOVIE = 'Sprite Fright (2021) [2160p HDR]';
const tv = (over: Partial<NonNullable<ItemView['torrent']>> = {}) => ({ infoHash: 'c0ffee'.padEnd(40, '0'), files: 6, chosenFiles: null, private: false, uploaded: 0, ratio: 0, seeding: false, ...over });
items.push(
  item({
    id: 't1', fileName: MOVIE, status: 'downloading', category: 'torrents', size: 7.4 * GB, downloaded: 3.1 * GB,
    kind: 'torrent', connections: 0, url: 'magnet:?xt=urn:btih:c0ffee', torrent: tv({ files: 13, chosenFiles: 12, uploaded: 410 * MB, ratio: 0.05 }),
  }),
  item({
    id: 't2', fileName: 'field-recordings-48k.flac', status: 'done', category: 'torrents', size: 840 * MB, downloaded: 840 * MB,
    kind: 'torrent', connections: 0, url: 'magnet:?xt=urn:btih:beef', addedAt: now - 8 * 3600_000, finishedAt: now - 7 * 3600_000,
    filePath: 'C:\\Users\\you\\Downloads\\Torrents\\field-recordings-48k.flac',
    torrent: tv({ infoHash: 'beef'.padEnd(40, '0'), files: 1, uploaded: 512 * MB, ratio: 0.61, seeding: true }),
  }),
);
const tsim = { t1: { pieces: Array.from({ length: 96 }, (_, i) => (i < 30 || (i > 50 && i < 58) || i % 7 === 0 ? 1 : i % 5 === 0 ? 0.5 : 0)), peers: 23, seeds: 9 } };
let mockTrackers: TrackerView[] = [
  { url: 'udp://tracker.example-one.org:6969/announce', tier: 0, state: 'working', seeders: 412, leechers: 61, peers: 50, lastAnnounce: now - 40_000, nextAnnounce: now + 1_760_000, message: null },
  { url: 'https://tracker.example-two.net/announce', tier: 0, state: 'working', seeders: 388, leechers: 57, peers: 50, lastAnnounce: now - 41_000, nextAnnounce: now + 1_200_000, message: null },
  { url: 'udp://open.example-three.io:1337/announce', tier: 1, state: 'notWorking', seeders: null, leechers: null, peers: 0, lastAnnounce: now - 30_000, nextAnnounce: now + 60_000, message: 'Timed out' },
  { url: 'http://tracker.example-four.com:80/announce', tier: 1, state: 'updating', seeders: null, leechers: null, peers: 0, lastAnnounce: null, nextAnnounce: null, message: null },
  { url: 'wss://tracker.example-web.com', tier: 2, state: 'disabled', seeders: null, leechers: null, peers: 0, lastAnnounce: null, nextAnnounce: null, message: 'wss:// trackers are not supported' },
];
// The film's folder as a release usually comes: the film, subtitles, extras, a sample.
const mockFiles = (
  [
    ['Sprite Fright (2021) [2160p HDR].mkv', 5.9 * GB, 0.38],
    ['Subs\\English.srt', 61 * 1024, 1],
    ['Subs\\English (SDH).srt', 68 * 1024, 1],
    ['Subs\\Hindi.srt', 94 * 1024, 1],
    ['Subs\\Spanish.srt', 63 * 1024, 0],
    ['Extras\\Making of Sprite Fright.mp4', 1.2 * GB, 0.22],
    ['Extras\\Trailer.mp4', 182 * MB, 1],
    ['Sample\\sample.mkv', 64 * MB, 0],
    ['poster.jpg', 1.4 * MB, 1],
    ['README.txt', 4 * 1024, 1],
    // Stills as uploaders add them: long names that only differ at the end.
    ['Stills + Media Info\\Sprite Fright (2021) [2160p HDR] - Media Info.txt', 26 * 1024, 1],
    ['Stills + Media Info\\Sprite Fright (2021) [2160p HDR] - Screenshot 01.png', 10.6 * MB, 1],
    ['Stills + Media Info\\Sprite Fright (2021) [2160p HDR] - Screenshot 02.png', 11.9 * MB, 0.5],
  ] as const
).map(([path, size, done], index) => ({ index, path: `${MOVIE}\\${path}`, size, done: size * done }));
let mockChosen: number[] | null = [0, 1, 2, 3, 4, 5, 6, 8, 9, 10, 11, 12];

// ?empty previews the first run: no downloads at all.
if (new URLSearchParams(location.search).has('empty')) {
  items.length = 0;
  sims.length = 0;
}

let settings: Settings = {
  torrentPort: 48213,
  torrentUpnp: true,
  torrentDht: true,
  torrentUtp: true,
  protectDownloads: true,
  uploadLimit: 0,
  seedRatio: 1,
  seedMinutes: 0,
  defaultTrackers: '',
  downloadDir: 'C:\\Users\\you\\Downloads',
  categoryFolders: true,
  maxConcurrent: 4,
  connections: 16,
  speedLimit: 0,
  closeToTray: true,
  notifyOnComplete: true,
  startWithWindows: false,
};

// ?notools previews the first video link, before yt-dlp and ffmpeg are installed.
let tools: ToolsStatus = {
  ready: !new URLSearchParams(location.search).has('notools'),
  ytDlp: '2026.10.26', ffmpeg: true, deno: true, installSize: 158_000_000, installing: null, error: null,
};

/** A synthetic thumbnail: a flat colour field, so no real media art is shipped in the preview. */
function fakeThumb(seed: number) {
  const hue = (seed * 67) % 360;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="320" height="180"><rect width="320" height="180" fill="hsl(${hue} 30% 28%)"/><circle cx="230" cy="70" r="46" fill="hsl(${hue} 40% 42%)"/><rect y="120" width="320" height="60" fill="hsl(${hue} 25% 18%)"/></svg>`;
  return `data:image/svg+xml;base64,${btoa(svg)}`;
}

function fakeVideo(url: string): VideoInfo {
  const audioOnly = /music/.test(url);
  const d = audioOnly ? 238 : 1268;
  const v = (height: number, compat: boolean, detail: string, size: number, rec = false) => ({
    choice: { kind: 'video' as const, height, compat }, label: `${height}p`, detail, ext: compat ? 'mp4' : height > 1080 ? 'mkv' : 'webm',
    size, approx: false, selector: 'x', note: null, recommended: rec,
  });
  const a = (format: 'm4a' | 'mp3' | 'opus' | 'flac', label: string, detail: string, size: number, note: string | null = null, rec = false) => ({
    choice: { kind: 'audio' as const, format }, label, detail, ext: format, size, approx: format === 'mp3' || format === 'flac', selector: 'x', note, recommended: rec,
  });
  return {
    url, title: audioOnly ? 'Harbour Lights - Low Tide (live)' : 'Building a tiny synth from scratch', channel: audioOnly ? 'Harbour Lights' : 'Bench Notes',
    duration: d, thumbnail: null, site: 'Example', isLive: false,
    options: [
      ...(audioOnly ? [] : [
        v(2160, false, 'MKV · AV1 · 60 fps · 4K · best quality', 2.9 * GB),
        v(1440, false, 'WebM · VP9 · 60 fps · best quality', 1.2 * GB),
        v(1080, false, 'WebM · VP9 · 60 fps · best quality', 612 * MB),
        v(1080, true, 'MP4 · H.264 · 60 fps · plays everywhere', 486 * MB, true),
        v(720, true, 'MP4 · H.264 · plays everywhere', 212 * MB),
        v(480, true, 'MP4 · H.264 · plays everywhere', 96 * MB),
        v(360, true, 'MP4 · H.264 · plays everywhere', 58 * MB),
      ]),
      a('m4a', 'M4A', 'AAC 130 kbps · original, not re-encoded', (d * 130_000) / 8, null, audioOnly),
      a('mp3', 'MP3', '320 kbps · converted, plays everywhere', d * 40_000),
      a('opus', 'Opus', 'Opus 135 kbps · original, not re-encoded', (d * 135_000) / 8),
      a('flac', 'FLAC', 'Lossless container', d * 110_000, 'The site only has lossy audio, so FLAC is a bigger file but does not sound better.'),
    ],
    playlist: /list=/.test(url) ? { title: 'Synth builds', count: 12 } : null,
    playlistOnly: false,
  };
}

function emit(e: CoreEvent) {
  listeners.forEach((l) => l(e));
}

setInterval(() => {
  const progress: Progress[] = [];
  for (const s of [...sims]) {
    const it = s.item;
    if (it.status !== 'downloading') continue;
    const sp = s.speed * BOOST * (0.85 + Math.random() * 0.3);
    // Each live line gets a wobbly share of the speed.
    for (const seg of s.segs) {
      const lineSpeed = (sp / s.segs.length) * (0.6 + Math.random() * 0.8);
      seg.speed = seg.speed ? seg.speed * 0.6 + lineSpeed * 0.4 : lineSpeed;
      seg.pos = Math.min(seg.end, seg.pos + lineSpeed / 4);
    }
    // Finished lines steal half of the biggest remaining part (work stealing).
    for (const seg of [...s.segs]) {
      if (seg.pos < seg.end) continue;
      s.segs.splice(s.segs.indexOf(seg), 1);
      const victim = [...s.segs].sort((a, b) => b.end - b.pos - (a.end - a.pos))[0];
      if (victim && victim.end - victim.pos > 8 * MB) {
        const mid = Math.floor(victim.pos + (victim.end - victim.pos) / 2);
        s.segs.push({ start: mid, pos: mid, end: victim.end, speed: 0 });
        victim.end = mid;
        s.total++;
      }
    }
    it.downloaded = it.size! - s.segs.reduce((a, g) => a + (g.end - g.pos), 0);
    const remaining = it.size! - it.downloaded;
    if (remaining <= 0) {
      it.status = 'done';
      it.finishedAt = Date.now();
      it.filePath = `${it.saveDir}\\${it.fileName}`;
      sims.splice(sims.indexOf(s), 1);
      emit({ type: 'upsert', item: { ...it } });
      emit({ type: 'completed', item: { ...it } });
      continue;
    }
    const size = it.size!;
    progress.push({
      id: it.id,
      downloaded: it.downloaded,
      size,
      speed: Math.round(sp),
      eta: Math.round(remaining / sp),
      activeConnections: s.segs.length,
      gaps: s.segs.filter((g) => g.pos < g.end).map((g) => [g.pos / size, g.end / size]),
      heads: s.segs.filter((g) => g.pos < g.end).map((g) => g.pos / size),
      parts: s.segs.map((g) => ({ start: g.start, pos: Math.floor(g.pos), end: g.end, speed: Math.round(g.speed) })),
      partsTotal: s.total,
      fileName: it.fileName,
    });
  }
  // Torrent swarm figures (synthetic).
  for (const it of items.filter((i) => i.kind === 'torrent' && (i.status === 'downloading' || i.torrent?.seeding))) {
    const live = it.status === 'downloading';
    const sp = live ? 6.2 * MB * BOOST * (0.8 + Math.random() * 0.4) : 0;
    if (live) it.downloaded = Math.min(it.size!, it.downloaded + sp / 4);
    const sim = (tsim as any)[it.id] ?? { pieces: Array(96).fill(1), peers: 4, seeds: 0 };
    if (live && Math.random() < 0.3) {
      const k = sim.pieces.findIndex((x: number) => x < 1);
      if (k >= 0) sim.pieces[k] = Math.min(1, sim.pieces[k] + 0.5);
    }
    progress.push({
      id: it.id, downloaded: it.downloaded, size: it.size, speed: Math.round(sp), eta: live && sp ? Math.round((it.size! - it.downloaded) / sp) : null,
      activeConnections: sim.peers, gaps: [], heads: [], parts: [], partsTotal: 0, fileName: it.fileName,
      torrent: { upSpeed: Math.round((live ? 380 : 120) * 1024 * (0.7 + Math.random() * 0.6)), uploaded: it.torrent!.uploaded, peers: sim.peers, seeds: sim.seeds, knownPeers: 410, checking: false, seeding: !live, pieces: [...sim.pieces] },
    });
  }
  const speed = progress.reduce((a, p) => a + p.speed, 0);
  const rem = progress.reduce((a, p) => a + ((p.size ?? 0) - p.downloaded), 0);
  emit({
    type: 'progress',
    items: progress,
    totals: { speed, active: progress.length, queued: items.filter((i) => i.status === 'queued').length, remainingBytes: rem, eta: speed ? Math.round(rem / speed) : null },
  });
}, 250);

export async function invoke<T>(cmd: string, args: Record<string, any> = {}): Promise<T> {
  const find = (id: string) => items.find((i) => i.id === id);
  const set = (ids: string[], status: ItemView['status']) =>
    ids.forEach((id) => {
      const it = find(id);
      if (!it) return;
      it.status = status;
      emit({ type: 'upsert', item: { ...it } });
    });
  switch (cmd) {
    case 'list_downloads':
      return items.map((i) => ({ ...i })) as T;
    case 'get_settings':
      return { ...settings } as T;
    case 'speed_history':
      // Synthetic last minute so the preview's trace starts full, like the app after reopening.
      return Array.from({ length: 240 }, (_, i) => Math.round(38 * MB * (0.8 + 0.15 * Math.sin(i / 9) + Math.random() * 0.1))) as T;
    case 'set_settings':
      settings = args.settings;
      return undefined as T;
    case 'probe_url': {
      await new Promise((r) => setTimeout(r, 450));
      const name = decodeURIComponent(new URL(args.url).pathname.split('/').pop() || 'download');
      if (/youtu\.?be|vimeo|soundcloud|\/watch/.test(args.url)) {
        return { final_url: args.url, file_name: name || 'watch', size: null, resumable: false, content_type: 'text/html; charset=utf-8' } as T;
      }
      return { final_url: args.url, file_name: name, size: 734 * MB, resumable: true, content_type: 'application/octet-stream' } as T;
    }
    case 'add_download': {
      const req = args.req as AddRequest;
      const name = req.fileName || decodeURIComponent(new URL(req.url).pathname.split('/').pop() || 'download');
      const it = item({ id: `n${Date.now()}`, fileName: name, status: 'downloading', category: 'other', size: 734 * MB, addedAt: Date.now(), url: req.url });
      addSim(it, 11 * MB, req.connections ?? 16);
      emit({ type: 'upsert', item: { ...it } });
      return it as T;
    }
    case 'clipboard_link':
      // ?clip=<url> previews the clipboard prefill.
      return (new URLSearchParams(location.search).get('clip') || null) as T;
    case 'pick_folder':
      return 'D:\\Media\\Downloads' as T;
    case 'tools_status':
      return { ...tools } as T;
    case 'install_tools': {
      // Simulated: three steps filling up over ~3 s (no real download in the preview).
      const sizes = [24, 91, 43].map((m) => m * MB);
      for (let i = 0; i < 3; i++) {
        for (let f = 0; f <= 10; f++) {
          tools = { ...tools, installing: { step: ['yt-dlp', 'ffmpeg', 'deno'][i], index: i, count: 3, downloaded: (sizes[i] * f) / 10, size: sizes[i] } };
          emit({ type: 'tools', status: { ...tools } });
          await new Promise((r) => setTimeout(r, 100));
        }
      }
      tools = { ...tools, ready: true, installing: null };
      emit({ type: 'tools', status: { ...tools } });
      return undefined as T;
    }
    case 'video_info':
      await new Promise((r) => setTimeout(r, 900));
      return fakeVideo(args.url) as T;
    case 'add_video': {
      const req = args.req as AddVideoRequest;
      const audio = req.option.choice.kind === 'audio';
      const it = item({
        id: `n${Date.now()}`, fileName: `${req.title}.${req.option.ext}`, status: 'downloading', category: audio ? 'music' : 'video',
        size: req.option.size ?? 200 * MB, addedAt: Date.now(), url: req.url, kind: 'video', connections: 1,
        media: { title: req.title, quality: req.option.label, format: req.option.ext.toUpperCase(), audioOnly: audio, duration: req.duration ?? null, channel: req.channel ?? null, site: req.site ?? null, thumb: true },
      });
      items.push(it);
      emit({ type: 'upsert', item: { ...it } });
      return [it] as T;
    }
    case 'thumbnail':
      return fakeThumb(String(args.id).length + String(args.id).charCodeAt(1)) as T;
    case 'torrent_preview': {
      await new Promise((r) => setTimeout(r, /^magnet/.test(args.link ?? '') ? 1400 : 500));
      return {
        infoHash: 'ab12'.padEnd(40, '0'), name: MOVIE, size: mockFiles.reduce((a, f) => a + f.size, 0),
        files: mockFiles.map((f) => ({ ...f, done: 0 })), trackers: mockTrackers.slice(0, 3).map((t) => t.url), private: false, pieceSize: 4 * MB, pieces: 1890,
      } as TorrentPreview as T;
    }
    case 'add_torrent': {
      const req = args.req as AddTorrentRequest;
      const chosen = req.onlyFiles ?? mockFiles.map((f) => f.index);
      const it = item({
        id: `n${Date.now()}`, fileName: 'Open Movie Collection (2026)', status: req.start ? 'downloading' : 'paused', category: 'torrents',
        size: mockFiles.filter((f) => chosen.includes(f.index)).reduce((a, f) => a + f.size, 0), addedAt: Date.now(), kind: 'torrent', connections: 0,
        url: 'magnet:?xt=urn:btih:ab12', torrent: tv({ chosenFiles: req.onlyFiles?.length ?? null }),
      });
      items.push(it);
      emit({ type: 'upsert', item: { ...it } });
      return it as T;
    }
    case 'torrent_trackers':
      return mockTrackers.map((t) => ({ ...t })) as T;
    case 'add_trackers': {
      const fresh = (args.urls as string[]).flatMap((u) => u.split(/\s+/)).filter((u) => /^(udp|https?):\/\//.test(u) && !mockTrackers.some((t) => t.url === u));
      const tier = Math.max(...mockTrackers.map((t) => t.tier), -1) + 1;
      mockTrackers = [...mockTrackers, ...fresh.map((url) => ({ url, tier, state: 'notContacted' as const, seeders: null, leechers: null, peers: 0, lastAnnounce: null, nextAnnounce: null, message: null }))];
      setTimeout(() => (mockTrackers = mockTrackers.map((t) => (fresh.includes(t.url) ? { ...t, state: 'working', seeders: 120, leechers: 14, peers: 30, lastAnnounce: Date.now(), nextAnnounce: Date.now() + 1_800_000 } : t))), 1500);
      return fresh.length as T;
    }
    case 'remove_trackers':
      mockTrackers = mockTrackers.filter((t) => !(args.urls as string[]).includes(t.url));
      return undefined as T;
    case 'replace_tracker':
      mockTrackers = mockTrackers.map((t) => (t.url === args.old ? { ...t, url: args.new, state: 'notContacted', seeders: null, leechers: null, peers: 0, message: null } : t));
      return undefined as T;
    case 'find_more_peers':
    case 'music_lookup': {
      await new Promise((r) => setTimeout(r, 600));
      const u = String((args as { url: string }).url);
      if (/amazon\./i.test(u)) {
        return { service: 'amazonMusic', kind: 'album', title: null, artist: null, album: null, year: null, duration: null, tracks: null, cover: null, manual: true, note: "DasIDM doesn't read Amazon Music pages. Type the artist and title to search the stores." } as T;
      }
      const album = /\/album\//.test(u);
      // A synthetic cover: two flat colour fields, no real album art in the preview.
      const cover = 'data:image/svg+xml;base64,' + btoa('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect width="64" height="64" fill="#1d3b53"/><circle cx="40" cy="26" r="14" fill="#ffd400"/><rect y="44" width="64" height="20" fill="#0b0d10"/></svg>');
      return {
        service: 'spotify', kind: album ? 'album' : 'track', title: album ? 'Night Harbour' : 'Paper Lanterns', artist: 'The Example Band',
        album: 'Night Harbour', year: 2019, duration: album ? null : 214, tracks: album ? 10 : null, cover, manual: false, note: null,
      } as T;
    }
    case 'music_stores':
      return [
        { id: 'bandcamp', name: 'Bandcamp', sells: 'Pick the format when you buy: MP3, FLAC, ALAC, AAC, Ogg, WAV and more' },
        { id: 'qobuz', name: 'Qobuz', sells: 'FLAC in CD quality, and 24-bit hi-res where the release has it' },
        { id: 'hdtracks', name: 'HDtracks', sells: 'Hi-res FLAC and ALAC' },
      ] as T;
    case 'open_store_search':
      return undefined as T;
    case 'upload_status':
      return { peakBps: 1_450_000, learnedSeconds: 600, learnSeconds: 60, capBps: 1_160_000 } as T;
    case 'public_trackers': {
      await new Promise((r) => setTimeout(r, 700));
      const list = ['udp://tracker.example-public.org:1337/announce', 'udp://open.example-tracker.net:6969/announce', 'https://tracker.example-best.io:443/announce'];
      if (cmd === 'public_trackers') return list as T;
      const fresh = list.filter((u) => !mockTrackers.some((t) => t.url === u));
      mockTrackers = [...mockTrackers, ...fresh.map((url) => ({ url, tier: 9, state: 'updating' as const, seeders: null, leechers: null, peers: 0, lastAnnounce: null, nextAnnounce: null, message: null }))];
      setTimeout(() => (mockTrackers = mockTrackers.map((t) => (fresh.includes(t.url) ? { ...t, state: 'working', seeders: 240, leechers: 31, peers: 50, lastAnnounce: Date.now(), nextAnnounce: Date.now() + 1_800_000 } : t))), 1500);
      return { added: fresh.length, total: list.length } as T;
    }
    case 'reannounce':
      mockTrackers = mockTrackers.map((t) => (t.state === 'disabled' ? t : { ...t, state: 'updating' }));
      setTimeout(() => (mockTrackers = mockTrackers.map((t) => (t.state === 'updating' ? { ...t, state: t.url.includes('three') ? 'notWorking' : 'working', lastAnnounce: Date.now() } : t))), 1200);
      return undefined as T;
    case 'torrent_files':
      if (args.id === 't2') return { files: [{ index: 0, path: 'field-recordings-48k.flac', size: 840 * MB, done: 840 * MB }], chosen: null } as T;
      // Chosen files keep filling, a few at a time, as peers send pieces.
      for (const f of mockFiles) {
        if (args.id === 't1' && (mockChosen ?? []).includes(f.index) && f.done < f.size) f.done = Math.min(f.size, f.done + f.size * 0.004 + 2 * MB);
      }
      return { files: mockFiles.map((f) => ({ ...f })), chosen: mockChosen } as T;
    case 'set_torrent_files':
      mockChosen = (args.only as number[]).length === mockFiles.length ? null : args.only;
      return undefined as T;
    case 'browser_status':
    case 'repair_browser':
      return { registered: true, manifest: 'C:\\Users\\you\\AppData\\Local\\DasIDM\\browser\\dev.dasidm.host.json', extensionDir: 'C:\\Users\\you\\AppData\\Local\\DasIDM\\browser\\firefox', error: null, lastSeen: Date.now() - 90_000 } as T;
    case 'open_extension_folder':
    case 'open_addon_page':
      return undefined as T;
    case 'seed':
      args.ids.forEach((id: string) => {
        const it = find(id);
        if (it?.torrent) {
          it.torrent = { ...it.torrent, seeding: true };
          emit({ type: 'upsert', item: { ...it } });
        }
      });
      return undefined as T;
    case 'pick_torrent_file':
      return 'C:\\Users\\you\\Downloads\\open-movies.torrent' as T;
    case 'register_torrent_handler':
      return undefined as T;
    case 'pause':
      // A finished torrent only stops seeding; it stays finished.
      for (const id of args.ids as string[]) {
        const it = find(id);
        if (it?.torrent && it.status === 'done') {
          it.torrent = { ...it.torrent, seeding: false };
          emit({ type: 'upsert', item: { ...it } });
        }
      }
      set((args.ids as string[]).filter((id) => find(id)?.status !== 'done'), 'paused');
      return undefined as T;
    case 'resume':
    case 'restart':
      set(args.ids, 'queued');
      return undefined as T;
    case 'remove':
      args.ids.forEach((id: string) => {
        const i = items.findIndex((x) => x.id === id);
        if (i >= 0) items.splice(i, 1);
      });
      emit({ type: 'removed', ids: args.ids });
      return undefined as T;
    default:
      return undefined as T;
  }
}

export async function listen(cb: (e: CoreEvent) => void) {
  listeners.push(cb);
}
