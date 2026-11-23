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
  kind: 'http' | 'video' | 'torrent';
  /** Video and music links: what was chosen. */
  media: MediaView | null;
  torrent: TorrentView | null;
}

export interface TorrentView {
  infoHash: string;
  files: number;
  /** How many files are chosen; null = all. */
  chosenFiles: number | null;
  private: boolean;
  uploaded: number;
  ratio: number;
  seeding: boolean;
}

export interface TorrentLive {
  upSpeed: number;
  uploaded: number;
  peers: number;
  seeds: number;
  knownPeers: number;
  checking: boolean;
  seeding: boolean;
  /** Verified share of each stretch of the torrent, 0..1. */
  pieces: number[];
}

export interface TorrentFile {
  index: number;
  /** Path inside the torrent, folders separated by \ */
  path: string;
  size: number;
  done: number;
}

export interface TorrentPreview {
  infoHash: string;
  name: string;
  size: number;
  files: TorrentFile[];
  trackers: string[];
  private: boolean;
  pieceSize: number;
  pieces: number;
}

export type TrackerState = 'notContacted' | 'updating' | 'working' | 'notWorking' | 'disabled';

export interface TrackerView {
  url: string;
  tier: number;
  state: TrackerState;
  seeders: number | null;
  leechers: number | null;
  /** Peers this tracker handed out. */
  peers: number;
  lastAnnounce: number | null;
  nextAnnounce: number | null;
  message: string | null;
}

export interface AddTorrentRequest {
  infoHash: string;
  onlyFiles?: number[] | null;
  extraTrackers: string[];
  saveDir?: string | null;
  start: boolean;
}

/** magnet: links, links to .torrent files, and local .torrent paths. */
export const isTorrentLink = (s: string) => /^magnet:\?/i.test(s.trim()) || /\.torrent(\?.*)?$/i.test(s.trim());
export const isLocalPath = (s: string) => /^[a-z]:\\/i.test(s.trim()) || s.trim().startsWith('\\\\');

export interface MediaView {
  title: string;
  /** "1080p", "MP3" */
  quality: string;
  /** "MP4", "MP3" */
  format: string;
  audioOnly: boolean;
  duration: number | null;
  channel: string | null;
  site: string | null;
  /** A thumbnail is stored; fetch it with api.thumbnail(id). */
  thumb: boolean;
}

export type VideoPhase = 'starting' | 'downloading' | 'merging' | 'converting' | 'finishing';

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
  /** Video jobs only. */
  phase?: VideoPhase | null;
  /** Torrents only. */
  torrent?: TorrentLive | null;
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
  torrentPort: number;
  torrentUpnp: boolean;
  torrentDht: boolean;
  uploadLimit: number;
  seedRatio: number;
  seedMinutes: number;
  defaultTrackers: string;
}

export interface ProbeInfo {
  final_url: string;
  file_name: string;
  size: number | null;
  resumable: boolean;
  content_type: string | null;
}

/** A link handed to the Add sheet: from the command line, or a download Firefox passed on
 * (with the headers that let the server give DasIDM the same file). */
export interface PendingAdd {
  url: string;
  headers: [string, string][];
  fileName: string | null;
  size: number | null;
  from: string | null;
}

export interface BrowserStatus {
  registered?: boolean;
  manifest?: string;
  extensionDir?: string;
  error?: string | null;
  lastSeen: number | null;
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

export type Choice = { kind: 'video'; height: number; compat: boolean } | { kind: 'audio'; format: 'm4a' | 'mp3' | 'opus' | 'flac' };

export interface VideoOption {
  choice: Choice;
  label: string;
  detail: string;
  ext: string;
  size: number | null;
  approx: boolean;
  selector: string;
  note: string | null;
  recommended: boolean;
}

export interface VideoInfo {
  url: string;
  title: string;
  channel: string | null;
  duration: number | null;
  thumbnail: string | null;
  site: string;
  isLive: boolean;
  options: VideoOption[];
  playlist: { title: string; count: number } | null;
  playlistOnly: boolean;
}

export interface AddVideoRequest {
  url: string;
  option: VideoOption;
  title: string;
  channel?: string | null;
  duration?: number | null;
  thumbnail?: string | null;
  site?: string | null;
  saveDir?: string | null;
  start: boolean;
  playlist: boolean;
}

export interface ToolsStatus {
  ready: boolean;
  ytDlp: string | null;
  ffmpeg: boolean;
  deno: boolean;
  installSize: number;
  installing: { step: string; index: number; count: number; downloaded: number; size: number | null } | null;
  error: string | null;
}

export const isWebPage = (contentType: string | null | undefined) =>
  !!contentType && /^(text\/html|application\/xhtml\+xml)\b/i.test(contentType.trim());

export type CoreEvent =
  | { type: 'upsert'; item: ItemView }
  | { type: 'removed'; ids: string[] }
  | { type: 'progress'; items: Progress[]; totals: Totals }
  | { type: 'completed'; item: ItemView }
  | { type: 'tools'; status: ToolsStatus };

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
