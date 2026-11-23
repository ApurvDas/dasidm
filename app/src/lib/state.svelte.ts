import { api } from './api';
import type { CoreEvent, ItemView, Progress, Settings, Status, ToolsStatus, Totals } from './types';

export type Filter = 'all' | 'active' | 'queued' | 'paused' | 'done' | 'failed';
export type SortKey = 'name' | 'type' | 'size' | 'status' | 'added';

const HISTORY = 240; // 60 s at 4 Hz

export const app = $state({
  items: {} as Record<string, ItemView>,
  progress: {} as Record<string, Progress>,
  totals: { speed: 0, active: 0, queued: 0, remainingBytes: 0, eta: null } as Totals,
  history: [] as number[],
  /** Rows that changed and have not been looked at yet (stay lit). */
  lit: {} as Record<string, 'moved' | 'done' | 'failed'>,
  selected: null as string | null,
  filter: 'all' as Filter,
  category: 'all',
  search: '',
  sort: { key: 'added' as SortKey, desc: true },
  settings: null as Settings | null,
  panel: 'pass' as 'pass' | 'settings',
  adding: false as boolean | string,
  confirmDelete: false,
  toast: null as string | null,
  /** Full-width parts view for the selected download. */
  partsOpen: false,
  /** Per download: stable part numbers (by start byte) and parts that were just split off. */
  partMeta: {} as Record<string, PartMeta>,
  /** yt-dlp, ffmpeg and deno: needed for video and music links. */
  tools: null as ToolsStatus | null,
  /** Thumbnails of video items, fetched once (data: URIs). */
  thumbs: {} as Record<string, string>,
});

const asked = new Set<string>();
/** The thumbnail for a video item, fetched on first use. */
export function thumbOf(item: ItemView): string | null {
  if (!item.media?.thumb) return null;
  if (app.thumbs[item.id]) return app.thumbs[item.id];
  if (!asked.has(item.id)) {
    asked.add(item.id);
    api.thumbnail(item.id).then((t) => {
      if (t) app.thumbs[item.id] = t;
    });
  }
  return null;
}

export interface PartMeta {
  next: number;
  /** start byte -> part number (#01, #02… in order of appearance) */
  nums: Record<number, number>;
  /** start byte -> number of the part it was cut from (0 = unknown); cleared after the flash */
  fresh: Record<number, number>;
}

/** Number new parts and detect splits: a new part starting where an existing part now ends was cut from it. */
function trackParts(p: Progress) {
  let meta = app.partMeta[p.id];
  const first = !meta;
  if (!meta) meta = app.partMeta[p.id] = { next: 1, nums: {}, fresh: {} };
  const sorted = [...p.parts].sort((a, b) => a.start - b.start);
  for (const part of sorted) {
    if (meta.nums[part.start] !== undefined) continue;
    meta.nums[part.start] = meta.next++;
    if (first) continue; // initial set is not news
    const victim = p.parts.find((o) => o.end === part.start && o.start !== part.start);
    meta.fresh[part.start] = victim ? (meta.nums[victim.start] ?? 0) : 0;
    const key = part.start;
    setTimeout(() => {
      const m = app.partMeta[p.id];
      if (m) delete m.fresh[key];
    }, 2600);
  }
}

export function matchesFilter(s: Status, f: Filter) {
  switch (f) {
    case 'all':
      return true;
    case 'active':
      return s === 'downloading';
    case 'queued':
      return s === 'queued';
    case 'paused':
      return s === 'paused';
    case 'done':
      return s === 'done';
    case 'failed':
      return s === 'failed';
  }
}

function onEvent(e: CoreEvent) {
  switch (e.type) {
    case 'upsert': {
      const prev = app.items[e.item.id];
      app.items[e.item.id] = e.item;
      if (prev && prev.status !== e.item.status) {
        if (e.item.status === 'failed') app.lit[e.item.id] = 'failed';
        if (e.item.status !== 'downloading') delete app.progress[e.item.id];
      }
      break;
    }
    case 'completed':
      app.lit[e.item.id] = 'done';
      break;
    case 'tools':
      app.tools = e.status;
      break;
    case 'removed':
      for (const id of e.ids) {
        delete app.items[id];
        delete app.progress[id];
        delete app.lit[id];
        delete app.partMeta[id];
        delete app.thumbs[id];
        if (app.selected === id) {
          app.selected = null;
          app.partsOpen = false;
        }
      }
      break;
    case 'progress': {
      const next: Record<string, Progress> = {};
      for (const p of e.items) {
        next[p.id] = p;
        trackParts(p);
      }
      app.progress = next;
      app.totals = e.totals;
      app.history.push(e.totals.speed);
      if (app.history.length > HISTORY) app.history.shift();
      break;
    }
  }
}

export async function init() {
  await api.listen(onEvent);
  const [list, settings, history, tools] = await Promise.all([
    api.list(),
    api.settings(),
    api.history().catch(() => []),
    api.toolsStatus().catch(() => null),
  ]);
  app.tools = tools;
  const map: Record<string, ItemView> = {};
  for (const i of list) map[i.id] = i;
  app.items = map;
  app.settings = settings;
  // The core kept the last minute while the window was closed; continue from it.
  app.history = [...history.slice(-HISTORY), ...app.history].slice(-HISTORY);
}

export function seen(id: string) {
  if (app.lit[id]) delete app.lit[id];
}

export function select(id: string | null) {
  app.selected = id;
  app.confirmDelete = false;
  app.panel = 'pass';
  if (id) seen(id);
}

let toastTimer: ReturnType<typeof setTimeout> | undefined;
export function toast(msg: string) {
  app.toast = msg;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (app.toast = null), 4000);
}

export async function run<T>(p: Promise<T>): Promise<T | undefined> {
  try {
    return await p;
  } catch (e) {
    toast(String(e));
    return undefined;
  }
}
