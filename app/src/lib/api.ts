import type { AddRequest, CoreEvent, ItemView, ProbeInfo, Settings } from './types';

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
  probe: (url: string) => invokeImpl<ProbeInfo>('probe_url', { url }),
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
  clipboardLink: () => invokeImpl<string | null>('clipboard_link'),
  pickFolder: (start?: string | null) => invokeImpl<string | null>('pick_folder', { start: start || null }),
  listen: (cb: (e: CoreEvent) => void) => listenImpl(cb),
};

export async function windowControls() {
  if (!inTauri) return null;
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  return getCurrentWindow();
}
