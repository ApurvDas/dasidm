export type Status = 'queued' | 'downloading' | 'paused' | 'done' | 'failed';

export interface ItemView {
  id: string;
  url: string;
  fileName: string;
  saveDir: string;
  filePath: string | null;
  size: number | null;
  downloaded: number;
  status: Status;
  category: string;
  addedAt: number;
  finishedAt: number | null;
  error: string | null;
  connections: number;
  resumable: boolean;
}

export interface Progress {
  id: string;
  downloaded: number;
  size: number | null;
  speed: number;
  eta: number | null;
  activeConnections: number;
  /** Missing ranges as [from, to] fractions. */
  gaps: [number, number][];
  /** Where each live connection is writing (fractions). */
  heads: number[];
  /** Parts being downloaded right now, one per live connection (bytes). */
  parts: PartView[];
  /** How many parts the file has been split into so far, finished ones included. */
  partsTotal: number;
  fileName: string;
}

export interface PartView {
  start: number;
  pos: number;
  end: number;
  speed: number;
}

export interface Totals {
  speed: number;
  active: number;
  queued: number;
  remainingBytes: number;
  eta: number | null;
}

export interface Settings {
  downloadDir: string;
  categoryFolders: boolean;
  maxConcurrent: number;
  connections: number;
  speedLimit: number;
  closeToTray: boolean;
  notifyOnComplete: boolean;
  startWithWindows: boolean;
}

export interface ProbeInfo {
  final_url: string;
  file_name: string;
  size: number | null;
  resumable: boolean;
  content_type: string | null;
}

export interface AddRequest {
  url: string;
  fileName?: string | null;
  saveDir?: string | null;
  connections?: number | null;
  headers?: [string, string][];
  start: boolean;
  /** Save the link even if it is a web page. */
  allowPage?: boolean;
}

export const isWebPage = (contentType: string | null | undefined) =>
  !!contentType && /^(text\/html|application\/xhtml\+xml)\b/i.test(contentType.trim());

export type CoreEvent =
  | { type: 'upsert'; item: ItemView }
  | { type: 'removed'; ids: string[] }
  | { type: 'progress'; items: Progress[]; totals: Totals }
  | { type: 'completed'; item: ItemView };

/** File types. `short` is the list's Type column; `name` is the chip and the pass. */
export const CATEGORIES: { id: string; name: string; short: string }[] = [
  { id: 'video', name: 'Video', short: 'Video' },
  { id: 'music', name: 'Music', short: 'Music' },
  { id: 'programs', name: 'Programs', short: 'Program' },
  { id: 'archives', name: 'Archives', short: 'Archive' },
  { id: 'discs', name: 'Disc images', short: 'Disc image' },
  { id: 'documents', name: 'Documents', short: 'Document' },
  { id: 'images', name: 'Images', short: 'Image' },
  { id: 'torrents', name: 'Torrents', short: 'Torrent' },
  { id: 'web', name: 'Web pages', short: 'Web page' },
  { id: 'other', name: 'Other', short: 'Other' },
];

export const category = (id: string | null | undefined) => CATEGORIES.find((c) => c.id === id) ?? CATEGORIES[CATEGORIES.length - 1];
/** The icon name for a file type. */
export const typeIcon = (id: string | null | undefined) => `type-${category(id).id}`;
