import { clock } from './format';
import type { ItemView, VideoPhase } from './types';

/** What yt-dlp is doing, in words. */
export const PHASE: Record<VideoPhase, string> = {
  starting: 'Starting',
  downloading: 'Downloading',
  merging: 'Joining video and sound',
  converting: 'Converting',
  finishing: 'Finishing',
};

/** Short board form. */
export const PHASE_BOARD: Record<VideoPhase, string> = {
  starting: 'STARTING',
  downloading: 'LOADING',
  merging: 'JOINING',
  converting: 'CONVERTING',
  finishing: 'FINISHING',
};

/** "1080p · 21:08" for video, "MP3 · 3:58" for music; null for plain files. */
export function mediaLine(i: ItemView): string | null {
  const m = i.media;
  if (!m) return null;
  const q = m.audioOnly ? m.format : m.quality;
  const len = clock(m.duration);
  return len ? `${q} · ${len}` : q;
}

const EXT: [string, RegExp][] = [
  ['video', /\.(mp4|mkv|webm|avi|mov|m4v|wmv|flv|ts|m2ts)$/i],
  ['music', /\.(mp3|flac|m4a|aac|ogg|opus|wav|wma|alac|ape)$/i],
  ['programs', /\.(exe|msi|msix|appx|bat|cmd)$/i],
  ['archives', /\.(zip|rar|7z|tar|gz|bz2|xz|zst)$/i],
  ['discs', /\.(iso|img|vhd|vhdx|dmg)$/i],
  ['documents', /\.(pdf|docx?|xlsx?|pptx?|txt|md|epub|csv|nfo)$/i],
  ['images', /\.(jpe?g|png|gif|webp|bmp|tiff?|heic|svg)$/i],
  ['torrents', /\.torrent$/i],
];
/** The file type of a name, by extension (the core does the same for downloads). */
export function categorize(name: string): string {
  return EXT.find(([, re]) => re.test(name))?.[0] ?? 'other';
}

/** After the streams arrive, yt-dlp joins or converts them; the bytes are all there by then. */
export const isPostPhase = (p: VideoPhase | null | undefined) => p === 'merging' || p === 'converting' || p === 'finishing';
