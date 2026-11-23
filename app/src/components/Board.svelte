<script lang="ts">
  import { PHASE_BOARD, isPostPhase } from '../lib/media';
  import { flip, type FlipParams } from 'svelte/animate';
  import { app, select, seen } from '../lib/state.svelte';
  import { bytes, doneAt, percent, speed, speedParts } from '../lib/format';
  import SegStrip from '../lib/SegStrip.svelte';
  import { reducedMotion } from '../lib/motion';
  import Icon from '../lib/Icon.svelte';
  import type { ItemView } from '../lib/types';

  const MAX_ROWS = 6;
  const HISTORY = 240;

  // The board re-ranks on its own clock (every 2 s, like a real board), not on every
  // speed wobble; and it holds still while the pointer is over it.
  let order = $state<string[]>([]);
  let hovering = $state(false);
  let lastRank: Record<string, number> = {};
  let rankedOnEta: Record<string, boolean> = {};
  const etaSince: Record<string, number> = {};
  const fade: Record<string, ReturnType<typeof setTimeout>> = {};

  /** Glide rows that changed places; a row whose size changed moved because the layout did
   *  (window maximised or resized), and gliding it from the old layout would squash the board. */
  function reorder(node: Element, rects: { from: DOMRect; to: DOMRect }, params: FlipParams) {
    const { from, to } = rects;
    if (Math.abs(from.height - to.height) > 0.5 || Math.abs(from.width - to.width) > 0.5) return { duration: 0 };
    return flip(node, rects, params);
  }

  function rank() {
    const loading = Object.values(app.items).filter((i) => i.status === 'downloading');
    // Hysteresis: start from the current order and let a row overtake only when it will
    // finish clearly sooner (15 s or 20%), so near-ties never swap on a speed wobble.
    const eta = (i: ItemView) => app.progress[i.id]?.eta ?? Number.POSITIVE_INFINITY;
    const prev = (i: ItemView) => lastRank[i.id] ?? 999;
    loading.sort((a, b) => prev(a) - prev(b) || eta(a) - eta(b) || a.addedAt - b.addedAt);
    const clearlySooner = (a: ItemView, b: ItemView) => {
      const ea = eta(a);
      const eb = eta(b);
      if (!isFinite(ea)) return false;
      if (!isFinite(eb)) return true;
      return ea < eb - Math.max(15, eb * 0.2);
    };
    for (let k = 1; k < loading.length; k++) {
      for (let j = k; j > 0 && clearlySooner(loading[j], loading[j - 1]); j--) {
        [loading[j - 1], loading[j]] = [loading[j], loading[j - 1]];
      }
    }
    const waiting = Object.values(app.items)
      .filter((i) => i.status === 'queued')
      .sort((a, b) => a.addedAt - b.addedAt);
    const next = [...loading, ...waiting].slice(0, MAX_ROWS).map((i) => i.id);
    // Only a real overtake lights a row: both rankings based on a known finish time, and the
    // row past its first 10 s (speeds ramp up while connections open; that is not news).
    const now = Date.now();
    const known = (id: string) => app.progress[id]?.eta != null;
    for (const id of next) if (known(id) && !etaSince[id]) etaSince[id] = now;
    const settled = (id: string) => etaSince[id] && now - etaSince[id] > 10_000;
    // A row overtook another only if it now sits above a row that was above it before
    // (rows leaving the board shift indexes; that is not an overtake).
    next.forEach((id, idx) => {
      if (lastRank[id] === undefined || !rankedOnEta[id] || !known(id) || !settled(id)) return;
      if (app.items[id]?.status !== 'downloading') return;
      const passed = next.slice(idx + 1).some((o) => lastRank[o] !== undefined && lastRank[o] < lastRank[id]);
      if (passed) {
        app.lit[id] = 'moved';
        clearTimeout(fade[id]);
        fade[id] = setTimeout(() => app.lit[id] === 'moved' && delete app.lit[id], 8000);
      }
    });
    lastRank = Object.fromEntries(next.map((id, i) => [id, i]));
    rankedOnEta = Object.fromEntries(next.map((id) => [id, known(id)]));
    order = next;
  }

  $effect(() => {
    const t = setInterval(() => !hovering && rank(), 2000);
    return () => clearInterval(t);
  });

  // New or finished rows join/leave immediately.
  $effect(() => {
    const ids = Object.values(app.items)
      .filter((i) => i.status === 'downloading' || i.status === 'queued')
      .map((i) => i.id)
      .join();
    void ids;
    if (!hovering) rank();
  });

  const rows = $derived(order.map((id) => app.items[id]).filter(Boolean));
  const hiddenCount = $derived(
    Object.values(app.items).filter((i) => i.status === 'downloading' || i.status === 'queued').length - rows.length,
  );
  const [bigNum, bigUnit] = $derived(speedParts(app.totals.speed));

  const trace = $derived.by(() => {
    // Right-aligned: the newest sample sits at the right edge; no fake zeros before data exists.
    const h = app.history;
    const max = Math.max(1, ...h);
    const step = 600 / (HISTORY - 1);
    const x0 = 600 - (h.length - 1) * step;
    return h.map((v, i) => `${(x0 + i * step).toFixed(1)},${(46 - (v / max) * 42).toFixed(1)}`).join(' ');
  });

  let online = $state(typeof navigator === 'undefined' ? true : navigator.onLine);
  $effect(() => {
    const up = () => (online = true);
    const down = () => (online = false);
    window.addEventListener('online', up);
    window.addEventListener('offline', down);
    return () => (window.removeEventListener('online', up), window.removeEventListener('offline', down));
  });

  function pctOf(i: ItemView) {
    const p = app.progress[i.id];
    return percent(p?.downloaded ?? i.downloaded, p?.size ?? i.size);
  }

  function status(i: ItemView) {
    if (i.status === 'queued') return 'WAITING';
    const p = app.progress[i.id];
    const pc = pctOf(i);
    if (!online) return pc == null ? 'NO NETWORK' : `NO NETWORK · ${pc}%`;
    if (i.kind === 'torrent') {
      if (p?.torrent?.checking) return `CHECKING · ${pc ?? 0}%`;
      if (!p || p.speed === 0) return pc == null ? 'FINDING PEERS' : `FINDING PEERS · ${pc}%`;
      return `${pc ?? '··'}% · ${p.torrent?.peers ?? 0} PEERS`;
    }
    if (i.kind === 'video') {
      if (p?.phase && isPostPhase(p.phase)) return PHASE_BOARD[p.phase];
      if (!p || p.speed === 0) return pc == null ? 'STARTING' : `STARTING · ${pc}%`;
      return `${pc ?? '··'}% · ${(i.media?.audioOnly ? i.media.format : i.media?.quality ?? '').toUpperCase()}`;
    }
    if (!p || p.speed === 0) return pc == null ? 'CONNECTING' : `CONNECTING · ${pc}%`;
    return `${pc ?? '··'}% · ${p.activeConnections} ${p.activeConnections === 1 ? 'LINE' : 'LINES'}`;
  }
</script>

<section class="board" aria-label="Departures: active downloads ranked by finish time">
  <header class="head">
    <div class="title">
      <h2>Departures</h2>
      {#if online}
        <span class="cap">{app.totals.active} loading · {app.totals.queued} waiting</span>
      {:else}
        <span class="cap offline">No network · downloads resume when you're back online</span>
      {/if}
    </div>
    <div class="trace" aria-hidden="true">
      <svg viewBox="0 0 600 48" preserveAspectRatio="none" class:idle={!app.history.some((v) => v > 0)}>
        <polyline points="{trace.split(' ')[0]?.split(',')[0] ?? 600},48 {trace} 600,48" class="area" />
        <polyline points={trace} class="line" />
      </svg>
    </div>
    <div class="figure">
      <span class="cap">Total speed</span>
      <span class="big">{bigNum}<small>{bigUnit}</small></span>
    </div>
    <div class="figure">
      <span class="cap">All done at</span>
      <span class="big">{app.totals.active ? doneAt(app.totals.eta) : '--:--'}<small>{bytes(app.totals.remainingBytes)} left</small></span>
    </div>
  </header>

  <div class="cols cap" role="row">
    <span>Done at</span><span>File</span><span class="r">Size</span><span class="r">Speed</span><span>Status</span>
  </div>

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="rows" onpointerenter={() => (hovering = true)} onpointerleave={() => ((hovering = false), rank())}>
    {#each rows as i (i.id)}
      {@const p = app.progress[i.id]}
      <button
        class="row"
        class:lit={app.lit[i.id] === 'moved'}
        class:waiting={i.status === 'queued'}
        class:sel={app.selected === i.id}
        animate:reorder={{ duration: reducedMotion() ? 0 : 520, easing: (t) => 1 - Math.pow(1 - t, 4) }}
        onclick={() => select(i.id)}
        onpointerenter={() => seen(i.id)}
      >
        <span class="time">{i.status === 'queued' ? '--:--' : doneAt(p?.eta)}</span>
        <span class="file" title={i.fileName}>{i.fileName}</span>
        <span class="r">{bytes(p?.size ?? i.size)}</span>
        <span class="r">{i.status === 'queued' ? '—' : speed(p?.speed)}</span>
        <span class="st">{#if app.lit[i.id] === 'moved'}<b><Icon name="caret" size={11} />MOVED UP</b> {pctOf(i) ?? '··'}%{:else}{status(i)}{/if}</span>
        <span class="strip">
          <SegStrip gaps={p?.gaps?.length ? p.gaps : null} heads={p?.heads ?? []} fraction={(p?.downloaded ?? i.downloaded) / ((p?.size ?? i.size) || 1)} tone={i.status === 'queued' ? 'steel' : 'ivory'} />
        </span>
      </button>
    {:else}
      <div class="empty">
        <span class="time">--:--</span>
        <span>No departures. Paste a link (Ctrl+V) or press <b>Add URL</b>.</span>
      </div>
    {/each}
    {#if hiddenCount > 0}
      <div class="more cap">+ {hiddenCount} more in the list below</div>
    {/if}
  </div>
</section>

<style>
  .board {
    container-type: inline-size;
    min-width: 0;
    overflow: hidden;
    background: var(--glass-board);
    box-shadow: var(--edge);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 14px 16px 10px;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .head {
    display: grid;
    grid-template-columns: auto minmax(120px, 1fr) auto auto;
    align-items: end;
    gap: 28px;
    padding-bottom: 12px;
  }
  h2 {
    font-family: var(--gothic);
    font-weight: 700;
    font-size: 34px;
    line-height: 1;
    letter-spacing: 0.03em;
    text-transform: uppercase;
    margin-bottom: 4px;
  }
  .offline {
    color: var(--alert);
  }
  .trace {
    height: 48px;
    align-self: center;
  }
  .trace svg {
    width: 100%;
    height: 100%;
    display: block;
  }
  .trace svg.idle {
    visibility: hidden;
  }
  .area {
    fill: rgb(242 242 238 / 0.07);
    stroke: none;
  }
  .line {
    fill: none;
    stroke: var(--ivory);
    stroke-width: 1.5;
    vector-effect: non-scaling-stroke;
    opacity: 0.8;
  }
  .figure {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 4px;
  }
  .big {
    font-family: var(--gothic);
    font-weight: 600;
    font-size: 46px;
    line-height: 0.9;
    letter-spacing: 0;
    display: flex;
    align-items: baseline;
    gap: 6px;
  }
  .big small {
    font-size: 12px;
    font-weight: 500;
    color: var(--fog);
    letter-spacing: 0.04em;
  }

  .cols,
  .row,
  .empty {
    display: grid;
    grid-template-columns: 86px minmax(0, 1fr) 96px 110px 190px;
    column-gap: 16px;
    align-items: center;
  }
  .cols {
    height: 26px;
    border-bottom: 1px solid var(--line-strong);
    padding: 0 10px;
  }
  .r {
    text-align: right;
  }
  .rows {
    overflow: hidden auto;
    min-height: 0;
  }
  .row {
    position: relative;
    width: 100%;
    height: 46px;
    padding: 0 10px;
    text-align: left;
    border-bottom: 1px solid var(--line);
    font-family: var(--mono);
    font-stretch: 75%;
    font-weight: 500;
    font-size: 16px;
    letter-spacing: 0.01em;
    transition: background 160ms var(--ease);
  }
  .row:hover {
    background: var(--hover);
  }
  .row.sel {
    background: var(--selected);
    box-shadow: var(--selected-line);
  }
  .row.lit {
    background: rgb(255 212 0 / 0.1);
    box-shadow: inset 0 0 0 1px rgb(255 212 0 / 0.45);
  }
  .row.waiting {
    color: var(--fog);
  }
  .time {
    font-family: var(--gothic);
    font-weight: 600;
    font-size: 22px;
    letter-spacing: 0.02em;
  }
  .waiting .time {
    color: var(--fog);
  }
  .file {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .st {
    font-size: 13px;
    letter-spacing: 0.06em;
    color: var(--fog);
    white-space: nowrap;
  }
  .st b {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    color: var(--alert-ink);
    background: var(--alert);
    padding: 3px 7px;
    border-radius: 3px;
    font-weight: 700;
  }
  .strip {
    position: absolute;
    left: 112px;
    right: 10px;
    bottom: 5px;
  }
  .empty {
    height: 46px;
    padding: 0 10px;
    color: var(--fog);
    grid-template-columns: 86px 1fr;
    font-family: var(--mono);
    font-stretch: 75%;
    font-size: 14px;
  }
  .empty .time {
    color: var(--fog);
  }
  /* Short windows: keep room for the full list below. */
  @media (max-height: 760px) {
    .rows {
      max-height: calc(3 * 46px);
    }
    .head {
      padding-bottom: 6px;
    }
  }
  .empty b {
    color: var(--ivory);
  }
  .more {
    padding: 8px 10px 0;
  }

  /* Narrow windows: the board keeps every column but tightens, and drops the trace. */
  @container (max-width: 820px) {
    .head {
      grid-template-columns: auto 1fr auto;
      gap: 18px;
    }
    .trace {
      display: none;
    }
    .big {
      font-size: 36px;
    }
    .cols,
    .row {
      grid-template-columns: 62px minmax(0, 1fr) 78px 92px 136px;
      column-gap: 10px;
    }
    .row {
      font-size: 14px;
    }
    .st {
      font-size: 12px;
      letter-spacing: 0.03em;
    }
    .strip {
      left: 82px;
    }
  }
</style>
