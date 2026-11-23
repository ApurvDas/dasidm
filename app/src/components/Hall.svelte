<script lang="ts">
  import { app, matchesFilter, select, type SortKey } from '../lib/state.svelte';
  import { bytes, percent, speed, when } from '../lib/format';
  import Mark from '../lib/Mark.svelte';
  import SegStrip from '../lib/SegStrip.svelte';
  import Icon from '../lib/Icon.svelte';
  import { CATEGORIES, type ItemView } from '../lib/types';

  const ROW = 34;
  let viewport = $state<HTMLDivElement>();
  let scrollTop = $state(0);
  let height = $state(400);
  let searchEl = $state<HTMLInputElement>();

  export function focusSearch() {
    searchEl?.focus();
    searchEl?.select();
  }

  const statusOrder = { downloading: 0, queued: 1, paused: 2, failed: 3, done: 4 } as const;

  const rows = $derived.by(() => {
    const q = app.search.trim().toLowerCase();
    const list = Object.values(app.items).filter(
      (i) =>
        matchesFilter(i.status, app.filter) &&
        (app.category === 'all' || i.category === app.category) &&
        (!q || i.fileName.toLowerCase().includes(q) || i.url.toLowerCase().includes(q)),
    );
    const { key, desc } = app.sort;
    const dir = desc ? -1 : 1;
    const cmp: Record<SortKey, (a: ItemView, b: ItemView) => number> = {
      name: (a, b) => a.fileName.localeCompare(b.fileName, undefined, { numeric: true }),
      size: (a, b) => (a.size ?? -1) - (b.size ?? -1),
      status: (a, b) => statusOrder[a.status] - statusOrder[b.status],
      added: (a, b) => a.addedAt - b.addedAt,
    };
    return list.sort((a, b) => cmp[key](a, b) * dir || b.addedAt - a.addedAt);
  });

  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW) - 6));
  const visible = $derived(rows.slice(first, first + Math.ceil(height / ROW) + 12));

  function sortBy(key: SortKey) {
    app.sort = app.sort.key === key ? { key, desc: !app.sort.desc } : { key, desc: key === 'added' || key === 'size' };
  }

  function statusText(i: ItemView) {
    const p = app.progress[i.id];
    switch (i.status) {
      case 'downloading': {
        const pc = percent(p?.downloaded ?? i.downloaded, p?.size ?? i.size);
        return `${pc ?? '··'}% · ${speed(p?.speed)}`;
      }
      case 'queued':
        return 'Waiting';
      case 'paused': {
        const pc = percent(i.downloaded, i.size);
        return pc == null ? 'Paused' : `Paused at ${pc}%`;
      }
      case 'failed':
        return i.error ?? 'Failed';
      case 'done':
        return `Finished ${when(i.finishedAt)}`;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
    e.preventDefault();
    e.stopPropagation();
    move(e.key === 'ArrowDown' ? 1 : -1);
  }

  /** Move the selection through the visible list (also used by the window-wide arrow keys). */
  export function move(delta: number) {
    const idx = rows.findIndex((r) => r.id === app.selected);
    const next = rows[idx < 0 ? 0 : Math.max(0, Math.min(rows.length - 1, idx + delta))];
    if (!next) return;
    select(next.id);
    const top = rows.indexOf(next) * ROW;
    if (viewport) {
      if (top < viewport.scrollTop) viewport.scrollTop = top;
      else if (top + ROW > viewport.scrollTop + height) viewport.scrollTop = top + ROW - height;
    }
  }

  const sortMark = (k: SortKey) => (app.sort.key === k ? (app.sort.desc ? 'down' : 'up') : null);
</script>

<section class="hall" aria-label="All downloads">
  <div class="bar">
    <label class="search">
      <Icon name="search" />
      <input bind:this={searchEl} class="field" placeholder="Search names and links" bind:value={app.search} spellcheck="false" />
      <kbd>Ctrl F</kbd>
    </label>
    <select class="field cat" bind:value={app.category} aria-label="Category">
      <option value="all">All types</option>
      {#each CATEGORIES as c}
        <option value={c.id}>{c.name}</option>
      {/each}
    </select>
    <span class="cap count">{rows.length} {rows.length === 1 ? 'item' : 'items'}</span>
  </div>

  <div class="thead cap" role="row">
    {#each [['name', 'Name'], ['size', 'Size'], ['status', 'Status'], ['added', 'Added']] as [k, label]}
      <span role="columnheader" class:r={k === 'size'} aria-sort={app.sort.key === k ? (app.sort.desc ? 'descending' : 'ascending') : 'none'}>
        <button onclick={() => sortBy(k as SortKey)}>
          {label}
          {#if sortMark(k as SortKey)}<Icon name={sortMark(k as SortKey)!} size={12} />{/if}
        </button>
      </span>
    {/each}
  </div>

  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    class="viewport"
    bind:this={viewport}
    bind:clientHeight={height}
    onscroll={(e) => (scrollTop = (e.currentTarget as HTMLDivElement).scrollTop)}
    onkeydown={onKey}
    tabindex="0"
    role="listbox"
    aria-label="Downloads"
  >
    <div class="spacer" style:height="{rows.length * ROW}px">
      {#each visible as i, n (i.id)}
        {@const p = app.progress[i.id]}
        <div
          class="tr {i.status}"
          class:sel={app.selected === i.id}
          class:lit={app.lit[i.id] === 'done' || app.lit[i.id] === 'failed'}
          style:transform="translateY({(first + n) * ROW}px)"
          role="option"
          aria-selected={app.selected === i.id}
          tabindex="-1"
          onclick={() => select(i.id)}
          ondblclick={() => i.status === 'done' && window.dispatchEvent(new CustomEvent('das:open', { detail: i.id }))}
          onkeydown={() => {}}
        >
          <span class="name">
            <Mark status={i.status} />
            <span class="text" title={i.fileName}>{i.fileName}</span>
          </span>
          <span class="r">{bytes(p?.size ?? i.size)}</span>
          <span class="st">
            {#if i.status === 'downloading' || i.status === 'paused'}
              <span class="mini"><SegStrip gaps={p?.gaps?.length ? p.gaps : null} heads={p?.heads ?? []} fraction={(p?.downloaded ?? i.downloaded) / ((p?.size ?? i.size) || 1)} tone={i.status === 'paused' ? 'steel' : 'ivory'} /></span>
            {/if}
            <span class="stext">{statusText(i)}</span>
          </span>
          <span class="added">{when(i.addedAt)}</span>
        </div>
      {/each}
    </div>
    {#if rows.length === 0}
      <div class="none">
        {#if Object.keys(app.items).length === 0}
          <p>Nothing downloaded yet.</p>
          <p class="hint">Copy a link and press <kbd>Ctrl V</kbd> here, or use <b>Add URL</b>.</p>
        {:else}
          <p>No downloads match.</p>
          <button class="btn" onclick={() => ((app.search = ''), (app.category = 'all'), (app.filter = 'all'))}>Clear filters</button>
        {/if}
      </div>
    {/if}
  </div>
</section>

<style>
  .hall {
    container-type: inline-size;
    min-width: 0;
    background: var(--glass-hall);
    box-shadow: var(--edge);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
  }
  .search {
    position: relative;
    flex: 1;
    max-width: 420px;
    color: var(--fog);
  }
  .search :global(svg) {
    position: absolute;
    left: 10px;
    top: 8px;
  }
  .search input {
    padding-left: 32px;
    padding-right: 64px;
  }
  .search kbd {
    position: absolute;
    right: 8px;
    top: 7px;
  }
  kbd {
    font-family: var(--mono);
    font-stretch: 75%;
    font-size: 10px;
    padding: 2px 5px;
    border-radius: 4px;
    border: 1px solid var(--line-strong);
    color: var(--fog);
  }
  .cat {
    width: 150px;
  }
  .count {
    margin-left: auto;
  }

  .thead,
  .tr {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 90px minmax(200px, 0.55fr) 130px;
    column-gap: 16px;
    align-items: center;
    padding: 0 14px;
  }
  .thead {
    height: 28px;
    border-top: 1px solid var(--line);
    border-bottom: 1px solid var(--line-strong);
  }
  .thead button {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font: inherit;
    letter-spacing: inherit;
    text-transform: inherit;
    color: inherit;
    height: 100%;
  }
  .thead button:hover {
    color: var(--ivory);
  }
  .thead span {
    display: flex;
    height: 100%;
  }
  .thead span.r {
    justify-content: flex-end;
  }
  .r {
    text-align: right;
  }
  .viewport {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow: hidden auto;
    outline: none;
  }
  .spacer {
    position: relative;
  }
  .tr {
    position: absolute;
    inset: 0 0 auto 0;
    height: 34px;
    border-bottom: 1px solid var(--line);
    contain: strict;
  }
  .tr:hover {
    background: var(--hover);
  }
  .tr.sel {
    background: var(--selected);
    box-shadow: var(--selected-line);
  }
  .tr.lit {
    box-shadow: inset 0 0 0 1px rgb(255 212 0 / 0.4);
  }
  .tr.done,
  .tr.queued {
    color: var(--fog);
  }
  .tr.done .name .text,
  .tr.sel .name .text {
    color: var(--ivory);
  }
  .name {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tr.downloading .name .text {
    font-weight: 600;
  }
  .st {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .mini {
    width: 72px;
    flex: none;
  }
  .stext {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .failed .stext {
    color: var(--bad);
  }
  .done .stext {
    color: var(--ok);
  }
  .added {
    color: var(--fog);
    white-space: nowrap;
  }
  .none {
    position: absolute;
    inset: 0;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 10px;
    color: var(--fog);
    text-align: center;
  }
  .none p:first-child {
    color: var(--ivory);
    font-size: 15px;
    font-weight: 600;
  }
  .none b {
    color: var(--ivory);
  }

  @container (max-width: 720px) {
    .thead,
    .tr {
      grid-template-columns: minmax(0, 1fr) 76px minmax(130px, 0.6fr);
      column-gap: 12px;
    }
    .thead > :nth-child(4),
    .added,
    .mini {
      display: none;
    }
    .cat {
      width: 120px;
    }
  }
</style>
