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
  kind: 'http' | 'video' | 'torrent' | 'convert';
  /** Video and music links: what was chosen. */
  media: MediaView | null;
  torrent: TorrentView | null;
  convert: ConvertView | null;
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
/** A Spotify or Amazon Music share link (mirrors `music::parse_link` in das-core). */
export const isMusicLink = (s: string) => {
  const t = s.trim();
  if (/^spotify:(track|album|playlist|artist):[A-Za-z0-9]+$/.test(t)) return true;
  try {
    const u = new URL(t);
    const h = u.hostname.toLowerCase();
    if (h === 'open.spotify.com') return /\/(track|album|playlist|artist)\/[A-Za-z0-9]+/.test(u.pathname);
    if (h === 'spotify.link') return u.pathname.length > 1;
    if (h.startsWith('music.amazon.')) return /^\/(albums|tracks|playlists|user-playlists|artists)\//.test(u.pathname);
  } catch {
    /* not a URL */
  }
  return false;
};

/** A conversion batch in the list. */
export interface ConvertView {
  preset: Preset;
  label: string;
  files: number;
  done: number;
  skipped: number;
  failed: number;
  note: string | null;
  keepsCover: boolean;
}

export type Preset =
  | 'mp3-320' | 'mp3-v0' | 'mp3-256' | 'mp3-192' | 'aac-256' | 'opus-160' | 'opus-128' | 'vorbis-q6'
  | 'alac' | 'flac' | 'flac-16' | 'wav-16';

/** The presets as the convert sheet offers them. */
export const PRESETS: { id: Preset; label: string; hint: string; lossless: boolean; kbps?: number }[] = [
  { id: 'mp3-320', label: 'MP3 320 kbps', hint: 'Plays everywhere', lossless: false, kbps: 320 },
  { id: 'mp3-v0', label: 'MP3 V0 (VBR)', hint: 'Near 320 quality, smaller', lossless: false, kbps: 245 },
  { id: 'mp3-256', label: 'MP3 256 kbps', hint: '', lossless: false, kbps: 256 },
  { id: 'mp3-192', label: 'MP3 192 kbps', hint: 'Smaller files', lossless: false, kbps: 192 },
  { id: 'aac-256', label: 'AAC 256 kbps (.m4a)', hint: 'iPhone and Apple apps', lossless: false, kbps: 256 },
  { id: 'opus-160', label: 'Opus 160 kbps', hint: 'Best quality per MB; no cover art', lossless: false, kbps: 160 },
  { id: 'opus-128', label: 'Opus 128 kbps', hint: 'Small phone files; no cover art', lossless: false, kbps: 128 },
  { id: 'vorbis-q6', label: 'Ogg Vorbis (q6)', hint: 'About 192 kbps; no cover art', lossless: false, kbps: 192 },
  { id: 'alac', label: 'ALAC (.m4a)', hint: 'Lossless for Apple Music', lossless: true },
  { id: 'flac', label: 'FLAC', hint: 'Lossless, same sample rate and bit depth', lossless: true },
  { id: 'flac-16', label: 'FLAC 16-bit, 44.1 or 48 kHz', hint: 'Hi-res made smaller; CD quality from 44.1 kHz sources', lossless: true },
  { id: 'wav-16', label: 'WAV 16-bit', hint: 'Uncompressed; no tags or cover in many players', lossless: true },
];

export type Verdict = { kind: 'ok' } | { kind: 'warn'; note: string } | { kind: 'skip'; note: string };

export interface PlannedFile {
  path: string;
  name: string;
  size: number;
  format: string | null;
  verdict: Verdict | null;
}

export interface ConvertPlan {
  files: PlannedFile[];
  outDir: string | null;
  keepsCover: boolean;
  ffmpegReady: boolean;
}

/** The public details of a music share link. */
export interface MusicInfo {
  service: 'spotify' | 'amazonMusic' | null;
  kind: 'track' | 'album' | 'playlist' | 'artist' | null;
  title: string | null;
  artist: string | null;
  album: string | null;
  year: number | null;
  duration: number | null;
  tracks: number | null;
  /** data: URI */
  cover: string | null;
  /** Nothing could be read: type artist and title. */
  manual: boolean;
  note: string | null;
}

/** A store that sells music as downloads. */
export interface MusicStore {
  id: string;
  name: string;
  sells: string;
}

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
  torrentUtp: boolean;
  protectDownloads: boolean;
  uploadLimit: number;
  seedRatio: number;
  seedMinutes: number;
  defaultTrackers: string;
}

/** The torrent upload cap while downloading, and the upload speed it's learned from. */
export interface UploadStatus {
  peakBps: number;
  learnedSeconds: number;
  learnSeconds: number;
  capBps: number;
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
