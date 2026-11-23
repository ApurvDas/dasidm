// SYNTHETIC preview data: only loaded when the UI runs in a plain browser (no Tauri),
// so the design can be reviewed without the Rust core. Never shipped as real content.
import type { AddRequest, CoreEvent, ItemView, Progress, Settings } from './types';

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

// ?empty previews the first run: no downloads at all.
if (new URLSearchParams(location.search).has('empty')) {
  items.length = 0;
  sims.length = 0;
}

let settings: Settings = {
  downloadDir: 'C:\\Users\\you\\Downloads',
  categoryFolders: true,
  maxConcurrent: 4,
  connections: 16,
  speedLimit: 0,
  closeToTray: true,
  notifyOnComplete: true,
  startWithWindows: false,
};

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
    case 'pause':
      set(args.ids, 'paused');
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
