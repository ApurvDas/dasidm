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

/** After the streams arrive, yt-dlp joins or converts them; the bytes are all there by then. */
export const isPostPhase = (p: VideoPhase | null | undefined) => p === 'merging' || p === 'converting' || p === 'finishing';
