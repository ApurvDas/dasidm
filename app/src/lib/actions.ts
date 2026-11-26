// What can be done to one download, in one place: the row menu, the keyboard and the Pass all
// use this list, so a download offers the same choices wherever it is.
import { api } from './api';
import { app, run, toast } from './state.svelte';
import type { ItemView } from './types';

export interface Action {
  id: string;
  label: string;
  icon: string;
  kbd?: string;
  danger?: boolean;
  run: () => unknown;
}

/** Ask before removing: `files` also deletes what is on disk. */
export function askRemove(ids: string[], files: boolean) {
  app.menu = null;
  app.ask = { ids, files };
}

/** The actions for a download, in groups (a separator goes between groups). */
export function actionsFor(item: ItemView): Action[][] {
  const id = item.id;
  const torrent = item.kind === 'torrent';
  const done = item.status === 'done';
  const main: Action[] = [];
  if (done) {
    main.push({ id: 'open', label: 'Open', icon: 'open', kbd: 'Enter', run: () => run(api.openFile(id)) });
  }
  if (item.status === 'downloading' || item.status === 'queued') {
    main.push({ id: 'pause', label: 'Pause', icon: 'pause', kbd: 'Space', run: () => run(api.pause([id])) });
  } else if (item.status === 'paused' || item.status === 'failed') {
    main.push({ id: 'resume', label: item.status === 'failed' ? 'Retry' : 'Resume', icon: 'play', kbd: 'Space', run: () => run(api.resume([id])) });
    if (!torrent) main.push({ id: 'restart', label: 'Start over', icon: 'restart', run: () => run(api.restart([id])) });
  }
  if (done && torrent) {
    main.push(
      item.torrent?.seeding
        ? { id: 'unseed', label: 'Stop seeding', icon: 'pause', run: () => run(api.pause([id])) }
        : { id: 'seed', label: 'Start seeding', icon: 'upload', run: () => run(api.seed([id])) },
    );
  }

  const more: Action[] = [{ id: 'folder', label: 'Show in folder', icon: 'folder', run: () => run(api.openFolder(id)) }];
  // Music you have: convert it to another format or quality.
  if (done && item.kind !== 'convert' && item.filePath && /\.(flac|wav|aiff?|alac|m4a|mp3|ogg|opus|wv|ape)$/i.test(item.filePath)) {
    more.push({ id: 'convert', label: 'Convert…', icon: 'type-music', run: () => (app.convert = [item.filePath!]) });
  }
  if (torrent || (item.status === 'downloading' && item.kind !== 'video' && item.kind !== 'convert')) {
    more.push({
      id: 'details',
      label: torrent ? 'Open torrent' : 'Open parts',
      icon: 'right',
      kbd: 'P',
      run: () => {
        app.selected = id;
        app.partsOpen = true;
      },
    });
  }
  if (item.url && /^(https?|magnet):/i.test(item.url)) {
    more.push({
      id: 'copy',
      label: torrent ? 'Copy magnet link' : 'Copy link',
      icon: 'copy',
      run: async () => {
        await navigator.clipboard.writeText(item.url);
        toast('Link copied');
      },
    });
  }

  const gone: Action[] = done
    ? [
        // Nothing is lost (the file stays), so no question.
        {
          id: 'remove',
          label: 'Remove from list',
          icon: 'close',
          run: async () => {
            await run(api.remove([id], false));
            if (app.selected === id) app.selected = null;
            toast(torrent ? 'Removed from the list; its files stay on your PC' : 'Removed from the list; the file stays on your PC');
          },
        },
        { id: 'delete', label: torrent && (item.torrent?.files ?? 1) > 1 ? 'Delete from list and PC (folder)…' : 'Delete from list and PC…', icon: 'trash', kbd: 'Shift Del', danger: true, run: () => askRemove([id], true) },
      ]
    : [{ id: 'remove', label: 'Remove and delete partial data…', icon: 'trash', kbd: 'Del', danger: true, run: () => askRemove([id], true) }];

  return [main, more, gone].filter((g) => g.length);
}
