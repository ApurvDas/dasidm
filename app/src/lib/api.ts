import type { AddRequest, AddTorrentRequest, AddVideoRequest, BrowserStatus, CoreEvent, ItemView, PendingAdd, ProbeInfo, Settings, ToolsStatus, TorrentFile, TorrentPreview, TrackerView, VideoInfo } from './types';

/** True inside the DasIDM window; false in a plain browser (design preview). */
export const inTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

type Invoke = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
let invokeImpl: Invoke;
let listenImpl: (cb: (e: CoreEvent) => void) => Promise<void>;

if (inTauri) {
  const core = await import('@tauri-apps/api/core');
  const event = await import('@tauri-apps/api/event');
  invokeImpl = core.invoke;
  listenImpl = async (cb) => {
    await event.listen<CoreEvent>('core', (e) => cb(e.payload));
  };
} else {
  const mock = await import('./mock');
  invokeImpl = mock.invoke;
  listenImpl = mock.listen;
}

export const api = {
  list: () => invokeImpl<ItemView[]>('list_downloads'),
  history: () => invokeImpl<number[]>('speed_history'),
  probe: (url: string, headers?: [string, string][] | null) => invokeImpl<ProbeInfo>('probe_url', { url, headers: headers?.length ? headers : null }),
  add: (req: AddRequest) => invokeImpl<ItemView>('add_download', { req }),
  pause: (ids: string[]) => invokeImpl<void>('pause', { ids }),
  resume: (ids: string[]) => invokeImpl<void>('resume', { ids }),
  restart: (ids: string[]) => invokeImpl<void>('restart', { ids }),
  remove: (ids: string[], deleteFiles: boolean) => invokeImpl<void>('remove', { ids, deleteFiles }),
  pauseAll: () => invokeImpl<void>('pause_all'),
  resumeAll: () => invokeImpl<void>('resume_all'),
  settings: () => invokeImpl<Settings>('get_settings'),
  setSettings: (settings: Settings) => invokeImpl<void>('set_settings', { settings }),
  openFile: (id: string) => invokeImpl<void>('open_file', { id }),
  openFolder: (id: string) => invokeImpl<void>('open_folder', { id }),
  toolsStatus: () => invokeImpl<ToolsStatus>('tools_status'),
  installTools: () => invokeImpl<void>('install_tools'),
  updateTools: () => invokeImpl<boolean>('update_tools'),
  videoInfo: (url: string) => invokeImpl<VideoInfo>('video_info', { url }),
  addVideo: (req: AddVideoRequest) => invokeImpl<ItemView[]>('add_video', { req }),
  thumbnail: (id: string) => invokeImpl<string | null>('thumbnail', { id }),
  // Torrents
  torrentPreview: (source: string) =>
    invokeImpl<TorrentPreview>('torrent_preview', /^[a-z]:\\/i.test(source) ? { link: null, path: source } : { link: source, path: null }),
  addTorrent: (req: AddTorrentRequest) => invokeImpl<ItemView>('add_torrent', { req }),
  trackers: (id: string) => invokeImpl<TrackerView[]>('torrent_trackers', { id }),
  addTrackers: (id: string, urls: string[]) => invokeImpl<number>('add_trackers', { id, urls }),
  removeTrackers: (id: string, urls: string[]) => invokeImpl<number>('remove_trackers', { id, urls }),
  replaceTracker: (id: string, old: string, next: string) => invokeImpl<void>('replace_tracker', { id, old, new: next }),
  reannounce: (id: string) => invokeImpl<void>('reannounce', { id }),
  torrentFiles: (id: string) => invokeImpl<{ files: TorrentFile[]; chosen: number[] | null }>('torrent_files', { id }),
  setTorrentFiles: (id: string, only: number[]) => invokeImpl<void>('set_torrent_files', { id, only }),
  seed: (ids: string[]) => invokeImpl<void>('seed', { ids }),
  pickTorrentFile: () => invokeImpl<string | null>('pick_torrent_file'),
  registerTorrentHandler: () => invokeImpl<void>('register_torrent_handler'),
  /** A link handed over (command line, Firefox), waiting for the Add sheet. */
  takePendingAdd: () => invokeImpl<PendingAdd | null>('take_pending_add'),
  onOpenAdd: async (cb: (p: PendingAdd) => void) => {
    if (!inTauri) return;
    const event = await import('@tauri-apps/api/event');
    await event.listen<PendingAdd>('open-add', (e) => cb(e.payload));
  },
  // Firefox
  browserStatus: () => invokeImpl<BrowserStatus>('browser_status'),
  repairBrowser: () => invokeImpl<BrowserStatus>('repair_browser'),
  openExtensionFolder: () => invokeImpl<void>('open_extension_folder'),
  openAddonPage: () => invokeImpl<void>('open_addon_page'),
  clipboardLink: () => invokeImpl<string | null>('clipboard_link'),
  pickFolder: (start?: string | null) => invokeImpl<string | null>('pick_folder', { start: start || null }),
  listen: (cb: (e: CoreEvent) => void) => listenImpl(cb),
};

export async function windowControls() {
  if (!inTauri) return null;
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  return getCurrentWindow();
}
