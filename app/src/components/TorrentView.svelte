<script lang="ts">
  // A departures board for one torrent: its trackers (editable while it runs), its files
  // (include or skip, live) and its pieces.
  import { api } from '../lib/api';
  import { app, run, toast } from '../lib/state.svelte';
  import { arrive } from '../lib/motion';
  import { bytes, doneAt, duration, percent, speed } from '../lib/format';
  import Icon from '../lib/Icon.svelte';
  import FileTree from './FileTree.svelte';
  import PieceMap from './PieceMap.svelte';
  import type { TorrentFile, TrackerState, TrackerView } from '../lib/types';

  const item = $derived(app.selected ? app.items[app.selected] : null);
  const p = $derived(item ? app.progress[item.id] : undefined);
  const live = $derived(p?.torrent);
  const size = $derived(p?.size ?? item?.size ?? 0);
  const loaded = $derived(p?.downloaded ?? item?.downloaded ?? 0);
  // Opens on what is being downloaded; the Pass can ask for another tab.
  let tab = $state<'trackers' | 'files' | 'pieces'>(app.torrentTab);

  // ---- trackers (polled while this view is open) ----
  let trackers = $state<TrackerView[]>([]);
  let now = $state(Date.now());
  $effect(() => {
    const id = item?.id;
    if (!id) return;
    let alive = true;
    const load = () => api.trackers(id).then((t) => alive && (trackers = t)).catch(() => {});
    load();
    const t = setInterval(() => {
      now = Date.now();
      if (tab === 'trackers') load();
    }, 1000);
    return () => {
      alive = false;
      clearInterval(t);
    };
  });
  let adding = $state('');
  let editing = $state<string | null>(null);
  let editValue = $state('');

  const STATE: Record<TrackerState, string> = {
    notContacted: 'Not contacted',
    updating: 'Updating…',
    working: 'Working',
    notWorking: 'Not working',
    disabled: 'Disabled',
  };
  const working = $derived(trackers.filter((t) => t.state === 'working').length);

  async function addTrackers() {
    if (!item || !adding.trim()) return;
    const n = await run(api.addTrackers(item.id, adding.split(/\s+/).filter(Boolean)));
    if (n === undefined) return;
    toast(n ? `${n} ${n === 1 ? 'tracker' : 'trackers'} added` : 'Those trackers are already in the list (or not valid)');
    adding = '';
    trackers = await api.trackers(item.id);
  }
  async function removeTracker(url: string) {
    if (!item) return;
    await run(api.removeTrackers(item.id, [url]));
    trackers = await api.trackers(item.id);
  }
  function startEdit(t: TrackerView) {
    editing = t.url;
    editValue = t.url;
  }
  async function saveEdit() {
    if (!item || !editing) return;
    if (editValue.trim() && editValue.trim() !== editing) {
      const ok = await run(api.replaceTracker(item.id, editing, editValue.trim()).then(() => true));
      if (!ok) return;
    }
    editing = null;
    trackers = await api.trackers(item.id);
  }
  async function reannounce() {
    if (!item) return;
    await run(api.reannounce(item.id));
    toast('Asking every tracker for peers now');
  }
  // More trackers hand out more peers: today's public list, added and asked in one go.
  let finding = $state(false);
  async function findMorePeers() {
    if (!item || finding) return;
    finding = true;
    const r = await run(api.findMorePeers(item.id));
    finding = false;
    if (!r) return;
    toast(r.added ? `Added ${r.added} public ${r.added === 1 ? 'tracker' : 'trackers'}; asking them all for peers` : `All ${r.total} public trackers are already here; asking them for peers again`);
    trackers = await api.trackers(item.id);
  }
  async function copyAll() {
    await navigator.clipboard.writeText(trackers.map((t) => t.url).join('\n'));
    toast(`Copied ${trackers.length} trackers`);
  }
  function when(ms: number | null) {
    if (!ms) return '—';
    const s = Math.round((ms - now) / 1000);
    return s > 0 ? `in ${duration(s)}` : 'now';
  }

  // ---- files ----
  let files = $state<TorrentFile[]>([]);
  let chosen = $state<Set<number>>(new Set());
  let saved = $state<Set<number>>(new Set());
  $effect(() => {
    const id = item?.id;
    if (!id || tab !== 'files') return;
    let alive = true;
    const load = (first: boolean) =>
      api.torrentFiles(id).then((r) => {
        if (!alive) return;
        files = r.files;
        const c = new Set(r.chosen ?? r.files.map((f) => f.index));
        saved = c;
        if (first) chosen = c;
      });
    load(true);
    const t = setInterval(() => load(false), 2000);
    return () => {
      alive = false;
      clearInterval(t);
    };
  });
  const dirty = $derived(chosen.size !== saved.size || [...chosen].some((i) => !saved.has(i)));
  async function saveFiles() {
    if (!item) return;
    const ok = await run(api.setTorrentFiles(item.id, [...chosen]).then(() => true));
    if (ok) {
      saved = new Set(chosen);
      toast('File choice saved');
    }
  }

  function close() {
    app.partsOpen = false;
  }
</script>

<section class="tview" use:arrive={{ y: 8 }} aria-label="Torrent {item?.fileName ?? ''}">
  <header>
    <div class="title">
      <h2>Torrent</h2>
      <span class="file" title={item?.fileName}>{item?.fileName ?? ''}</span>
    </div>
    <div class="fig"><span class="cap">Peers · seeds</span><span class="big">{live?.peers ?? 0}<small> · {live?.seeds ?? 0}</small></span></div>
    <div class="fig"><span class="cap">Down</span><span class="big">{speed(p?.speed).replace('/s', '')}<small>/s</small></span></div>
    <div class="fig"><span class="cap">Up</span><span class="big">{speed(live?.upSpeed).replace('/s', '')}<small>/s</small></span></div>
    <div class="fig"><span class="cap">Ratio</span><span class="big">{(item?.torrent?.ratio ?? 0).toFixed(2)}</span></div>
    <div class="fig"><span class="cap">Done at</span><span class="big">{item?.status === 'downloading' ? doneAt(p?.eta) : '--:--'}</span></div>
    <button class="btn ghost" onclick={close}><Icon name="close" />Back to list<kbd>Esc</kbd></button>
  </header>

  <nav class="tabs" role="tablist" aria-label="Torrent details">
    <button role="tab" aria-selected={tab === 'trackers'} class:on={tab === 'trackers'} onclick={() => (tab = 'trackers')}>
      Trackers <span class="n">{working}/{trackers.length}</span>
    </button>
    <button role="tab" aria-selected={tab === 'files'} class:on={tab === 'files'} onclick={() => (tab = 'files')}>
      Files <span class="n">{item?.torrent?.chosenFiles ?? item?.torrent?.files ?? 0}/{item?.torrent?.files ?? 0}</span>
    </button>
    <button role="tab" aria-selected={tab === 'pieces'} class:on={tab === 'pieces'} onclick={() => (tab = 'pieces')}>
      Pieces <span class="n">{percent(loaded, size) ?? 0}%</span>
    </button>
  </nav>

  {#if tab === 'trackers'}
    <div class="cols cap trk" role="row">
      <span>Status</span><span>Tracker</span><span class="r">Tier</span><span class="r">Seeds</span><span class="r">Peers</span><span class="r">Got</span><span>Next</span><span></span>
    </div>
    <div class="rows" role="list">
      <!-- Peer sources that are not trackers, so the list explains where peers come from. -->
      <div class="row trk src" role="listitem">
        <span class="st"><i class="dot" class:ok={!item?.torrent?.private && app.settings?.torrentDht}></i>{item?.torrent?.private ? 'Off' : app.settings?.torrentDht ? 'On' : 'Off'}</span>
        <span class="url"
          ><span class="u">DHT, peer exchange and local network</span><span class="msg"
            >{item?.torrent?.private ? 'Off for private torrents: trackers only' : `${live?.knownPeers ?? 0} peers known from all sources`}</span
          ></span
        >
        <span></span><span></span><span></span><span class="r">{live?.knownPeers ?? 0}</span><span></span><span></span>
      </div>
      {#each trackers as t (t.url)}
        <div class="row trk" class:bad={t.state === 'notWorking'} class:dim={t.state === 'disabled'} role="listitem">
          <span class="st"><i class="dot {t.state}"></i>{STATE[t.state]}</span>
          {#if editing === t.url}
            <!-- svelte-ignore a11y_autofocus -->
            <input
              class="field edit"
              bind:value={editValue}
              autofocus
              spellcheck="false"
              onkeydown={(e) => {
                if (e.key === 'Enter') saveEdit();
                if (e.key === 'Escape') {
                  e.stopPropagation();
                  editing = null;
                }
              }}
            />
          {:else}
            <span class="url" title={t.message ? `${t.url}\n${t.message}` : t.url}>
              <span class="u">{t.url}</span>
              {#if t.message}<span class="msg">{t.message}</span>{/if}
            </span>
          {/if}
          <span class="r">{t.tier}</span>
          <span class="r">{t.seeders ?? '—'}</span>
          <span class="r">{t.leechers ?? '—'}</span>
          <span class="r">{t.peers}</span>
          <span class="next">{t.state === 'disabled' ? '—' : when(t.nextAnnounce)}</span>
          <span class="acts">
            {#if editing === t.url}
              <button class="btn ghost sm" onclick={saveEdit} aria-label="Save tracker"><Icon name="check" size={14} /></button>
            {:else}
              <button class="btn ghost sm" onclick={() => startEdit(t)} aria-label="Edit {t.url}" title="Edit"><Icon name="edit" size={14} /></button>
              <button class="btn ghost sm" onclick={() => removeTracker(t.url)} aria-label="Remove {t.url}" title="Remove"><Icon name="trash" size={14} /></button>
            {/if}
          </span>
        </div>
      {:else}
        <div class="none">No trackers yet. Peers come from DHT{item?.torrent?.private ? ' (off for private torrents)' : ''}; add trackers below to find more.</div>
      {/each}
    </div>
    <div class="addbar">
      <textarea class="field" bind:value={adding} rows="2" spellcheck="false" placeholder="Add trackers: one or more addresses, one per line"></textarea>
      <div class="btns">
        <button class="btn primary" disabled={!adding.trim()} onclick={addTrackers}><Icon name="plus" />Add</button>
        <button class="btn" onclick={findMorePeers} disabled={finding || !!item?.torrent?.private} title={item?.torrent?.private ? 'Private torrents use only their own trackers' : 'Adds the public tracker list from github.com/ngosang/trackerslist'}
          ><Icon name="search" />{finding ? 'Finding…' : 'Find more peers'}</button
        >
        <button class="btn" onclick={reannounce}><Icon name="restart" />Ask all now</button>
        <button class="btn ghost" disabled={!trackers.length} onclick={copyAll}><Icon name="copy" />Copy all</button>
      </div>
    </div>
  {:else if tab === 'files'}
    <div class="cols cap fl" role="row"><span>Folders and files · {bytes(files.filter((f) => chosen.has(f.index)).reduce((a, f) => a + f.size, 0))} chosen</span><span class="r">On disk</span><span class="r">Size</span></div>
    <div class="rows">
      <FileTree {files} root={item?.fileName} bind:chosen editable progress showRoot />
    </div>
    <div class="addbar files">
      <span class="hint">Skipped files are not downloaded; their pieces that touch chosen files still arrive.</span>
      <button class="btn primary" disabled={!dirty || chosen.size === 0} onclick={saveFiles}><Icon name="check" />Save choice</button>
    </div>
  {:else}
    <div class="pieces">
      <div class="maphead cap">
        <span>{bytes(loaded)} of {bytes(size)} verified · {percent(loaded, size) ?? 0}%</span>
        <span class="legend"><i class="k on"></i>Verified <i class="k off"></i>Missing</span>
      </div>
      <PieceMap pieces={live?.pieces} fallback={size ? loaded / size : 0} height={160} />
      <p class="explain">
        Each bar is one stretch of the torrent, filled as far as its pieces are checked against the torrent's hashes. Peers send pieces
        in any order, so the map fills in scattered, not left to right.{#if live?.checking} DasIDM is checking pieces already on disk.{/if}
      </p>
    </div>
  {/if}
</section>

<style>
  .tview {
    position: absolute;
    inset: 0;
    z-index: 5;
    background: var(--glass-board);
    box-shadow: var(--edge);
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    padding: 14px 16px 10px;
    display: flex;
    flex-direction: column;
    min-height: 0;
    container-type: inline-size;
  }
  header {
    display: grid;
    grid-template-columns: minmax(0, 1fr) repeat(5, auto) auto;
    gap: 24px;
    align-items: end;
    padding-bottom: 12px;
  }
  .title {
    min-width: 0;
  }
  h2 {
    font-family: var(--gothic);
    font-weight: 700;
    font-size: 34px;
    line-height: 1;
    letter-spacing: 0.03em;
    text-transform: uppercase;
  }
  .file {
    display: block;
    margin-top: 4px;
    font-family: var(--mono);
    font-stretch: 75%;
    font-size: 13px;
    color: var(--fog);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .fig {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 4px;
  }
  .big {
    font-family: var(--gothic);
    font-weight: 600;
    font-size: 30px;
    line-height: 0.9;
    white-space: nowrap;
  }
  .big small {
    font-size: 14px;
    color: var(--fog);
    margin-left: 2px;
  }
  kbd {
    font-family: var(--mono);
    font-stretch: 75%;
    font-size: 10px;
    padding: 1px 4px;
    border-radius: 3px;
    border: 1px solid currentColor;
    opacity: 0.55;
  }
  .tabs {
    display: flex;
    gap: 2px;
    padding-bottom: 10px;
  }
  .tabs button {
    height: 30px;
    padding: 0 12px;
    border-radius: 6px;
    display: flex;
    align-items: center;
    gap: 7px;
    color: var(--fog);
    font-weight: 600;
  }
  .tabs button:hover {
    color: var(--ivory);
    background: var(--hover);
  }
  .tabs button.on {
    color: var(--ivory);
    background: rgb(255 255 255 / 0.09);
  }
  .n {
    font-family: var(--mono);
    font-stretch: 75%;
    font-size: 11px;
    color: var(--fog);
  }
  .cols,
  .row {
    display: grid;
    column-gap: 14px;
    align-items: center;
    padding: 0 10px;
  }
  .trk {
    grid-template-columns: 118px minmax(0, 1fr) 36px 56px 56px 48px 88px 64px;
  }
  /* Lines up with FileTree's meter (84 + 8 + 44) and size (70) columns. */
  .cols.fl {
    grid-template-columns: minmax(0, 1fr) 136px 70px;
    column-gap: 8px;
    padding: 0 12px 0 6px;
  }
  .cols {
    height: 26px;
    border-bottom: 1px solid var(--line-strong);
  }
  .r {
    text-align: right;
  }
  .rows {
    flex: 1;
    min-height: 0;
    overflow: hidden auto;
  }
  .row {
    min-height: 40px;
    border-bottom: 1px solid var(--line);
    font-family: var(--mono);
    font-stretch: 75%;
    font-size: 13px;
  }
  .row.src {
    color: var(--fog);
  }
  /* A failing tracker reads at a glance: a red line and its reason. */
  .row.bad {
    background: rgb(255 107 107 / 0.07);
    box-shadow: inset 3px 0 0 var(--bad);
  }
  .row.dim {
    color: var(--steel);
  }
  .st {
    display: flex;
    align-items: center;
    gap: 8px;
    font-family: var(--body);
    font-stretch: normal;
    font-size: 12.5px;
    font-weight: 600;
  }
  .bad .st {
    color: var(--bad);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--steel);
    flex: none;
  }
  .dot.ok,
  .dot.working {
    background: var(--ok);
  }
  .dot.notWorking {
    background: var(--bad);
  }
  .dot.updating {
    background: var(--ivory);
    animation: pulse 1s ease-in-out infinite alternate;
  }
  @keyframes pulse {
    to {
      opacity: 0.3;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .dot.updating {
      animation: none;
    }
  }
  .url {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  /* Tracker rows: the address, and under it the reason when there is one. */
  .trk .url {
    flex-direction: column;
    align-items: flex-start;
    justify-content: center;
    gap: 1px;
    padding: 4px 0;
  }
  .u {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .url :global(svg) {
    flex: none;
    color: var(--fog);
  }
  .msg {
    max-width: 100%;
    font-family: var(--body);
    font-stretch: normal;
    font-size: 12px;
    color: var(--fog);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .bad .msg {
    color: var(--bad);
  }
  .next {
    color: var(--fog);
  }
  .acts {
    display: flex;
    justify-content: flex-end;
    gap: 2px;
  }
  .sm {
    height: 26px;
    padding: 0 6px;
  }
  .edit {
    height: 28px;
    font-family: var(--mono);
    font-stretch: 75%;
    font-size: 13px;
  }
  .none {
    padding: 24px 10px;
    color: var(--fog);
  }
  .addbar {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding-top: 10px;
    border-top: 1px solid var(--line-strong);
  }
  .addbar textarea {
    flex: 1;
    height: auto;
    resize: vertical;
    padding: 8px 10px;
    font-family: var(--mono);
    font-stretch: 75%;
    font-size: 12.5px;
  }
  .btns {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .addbar.files {
    justify-content: space-between;
    align-items: center;
  }
  .hint {
    color: var(--fog);
  }
  .pieces {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .maphead {
    display: flex;
    justify-content: space-between;
  }
  .legend {
    display: flex;
    gap: 6px;
    align-items: center;
    text-transform: none;
  }
  .k {
    width: 10px;
    height: 10px;
    border-radius: 2px;
    margin-left: 10px;
  }
  .k.on {
    background: rgb(242 242 238 / 0.82);
  }
  .k.off {
    background: #15191e;
    box-shadow: inset 0 0 0 1px var(--line-strong);
  }
  .explain {
    color: var(--fog);
    max-width: 75ch;
  }
  @container (max-width: 820px) {
    header {
      grid-template-columns: minmax(0, 1fr) repeat(3, auto) auto;
    }
    header .fig:nth-of-type(4),
    header .fig:nth-of-type(5) {
      display: none;
    }
    .trk {
      grid-template-columns: 104px minmax(0, 1fr) 48px 48px 64px;
    }
    .trk > :nth-child(3),
    .trk > :nth-child(6),
    .trk > :nth-child(7) {
      display: none;
    }
  }
</style>
