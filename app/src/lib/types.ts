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
}

export type CoreEvent =
  | { type: 'upsert'; item: ItemView }
  | { type: 'removed'; ids: string[] }
  | { type: 'progress'; items: Progress[]; totals: Totals }
  | { type: 'completed'; item: ItemView };

export const CATEGORIES: { id: string; name: string }[] = [
  { id: 'video', name: 'Video' },
  { id: 'music', name: 'Music' },
  { id: 'programs', name: 'Programs' },
  { id: 'archives', name: 'Archives' },
  { id: 'discs', name: 'Disc images' },
  { id: 'documents', name: 'Documents' },
  { id: 'images', name: 'Images' },
  { id: 'torrents', name: 'Torrents' },
  { id: 'other', name: 'Other' },
];
