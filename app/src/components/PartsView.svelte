<script lang="ts">
  // A departures board for one file: every connection is a row, in file order.
  import { app } from '../lib/state.svelte';
  import { arrive, closeRow, glide, openRow } from '../lib/motion';
  import { bytes, doneAt, percent, speed } from '../lib/format';
  import PartsMap from './PartsMap.svelte';
  import Icon from '../lib/Icon.svelte';
  import type { PartView } from '../lib/types';

  const item = $derived(app.selected ? app.items[app.selected] : null);
  const p = $derived(item ? app.progress[item.id] : undefined);
  const meta = $derived(item ? app.partMeta[item.id] : undefined);
  // A part that just finished stays one beat at 100% before its row closes.
  let ghosts = $state<Record<number, PartView>>({});
  let prevLive: PartView[] = [];
  $effect(() => {
    const live = p?.parts ?? [];
    const keys = new Set(live.map((x) => x.start));
    for (const old of prevLive) {
      if (!keys.has(old.start) && !ghosts[old.start]) {
        ghosts[old.start] = { ...old, pos: old.end, speed: 0 };
        setTimeout(() => delete ghosts[old.start], 700);
      }
    }
    prevLive = live;
  });
  const parts = $derived(
    p ? [...p.parts, ...Object.values(ghosts).filter((g) => !p.parts.some((x) => x.start === g.start))].sort((a, b) => a.start - b.start) : [],
  );
  const size = $derived(p?.size ?? item?.size ?? 0);
  const loaded = $derived(p?.downloaded ?? item?.downloaded ?? 0);

  /** Range in one unit so a column of ranges reads cleanly. */
  function range(a: number, b: number) {
    const unit = size >= 1024 ** 3 ? 1024 ** 3 : size >= 1024 ** 2 ? 1024 ** 2 : 1024;
    const label = unit === 1024 ** 3 ? 'GB' : unit === 1024 ** 2 ? 'MB' : 'KB';
    const d = unit === 1024 ? 0 : 2;
    return `${(a / unit).toFixed(d)}–${(b / unit).toFixed(d)} ${label}`;
  }
  const no = (start: number) => String(meta?.nums[start] ?? 0).padStart(2, '0');

  function close() {
    app.partsOpen = false;
  }
</script>

<section class="parts" use:arrive={{ y: 8 }} aria-label="Parts of {item?.fileName ?? 'download'}">
  <header>
    <div class="title">
      <h2>Parts</h2>
      <span class="file" title={item?.fileName}>{item?.fileName ?? ''}</span>
    </div>
    <div class="fig"><span class="cap">Lines live</span><span class="big">{p?.parts.length ?? 0}</span></div>
    <div class="fig"><span class="cap">Parts so far</span><span class="big">{p?.partsTotal ?? 0}</span></div>
    <div class="fig"><span class="cap">Speed</span><span class="big">{speed(p?.speed).replace('/s', '')}<small>/s</small></span></div>
    <div class="fig"><span class="cap">Done at</span><span class="big">{item?.status === 'downloading' ? doneAt(p?.eta) : '--:--'}</span></div>
    <button class="btn ghost" onclick={close}><Icon name="close" />Back to list<kbd>Esc</kbd></button>
  </header>

  <div class="mapwrap">
    <div class="maphead cap">
      <span>File map · {bytes(loaded)} of {bytes(size)} · {percent(loaded, size) ?? 0}%</span>
      <span class="legend"><i class="k loaded"></i>On disk <i class="k miss"></i>Still to load <i class="k head"></i>Writing now</span>
    </div>
    <PartsMap {p} fallback={size ? loaded / size : 0} height={64} labels />
    <p class="explain">
      DasIDM splits the file into parts and loads them on separate connections at the same time. When a line finishes early it takes over
      half of the biggest part that is left; that cut flashes yellow.
    </p>
  </div>

  <div class="cols cap" role="row">
    <span>Part</span><span>Range</span><span>Loaded</span><span class="r">Done</span><span class="r">Speed</span>
  </div>
  <div class="rows" role="list">
    {#each parts as part (part.start)}
      {@const from = meta?.fresh[part.start]}
      {@const len = Math.max(1, part.end - part.start)}
      <div class="row" class:new={from !== undefined} role="listitem" use:openRow out:closeRow>
        <span class="no">#{no(part.start)}</span>
        <span class="range">
          <span class="rt">{range(part.start, part.end)}</span>
          {#if from !== undefined}
            <b class="split" title={from ? `Split from part #${String(from).padStart(2, '0')}` : 'New line'}>
              {#if from}<span class="long">SPLIT FROM</span><span class="short"><Icon name="left" size={11} /></span>#{String(from).padStart(2, '0')}{:else}NEW LINE{/if}
            </b>
          {/if}
        </span>
        <span class="track">
          <span class="fill" use:glide={{ prop: 'width', value: ((part.pos - part.start) / len) * 100 }}></span>
        </span>
        <span class="r pc">{part.pos >= part.end ? 100 : Math.min(99, Math.floor(((part.pos - part.start) / len) * 100))}%</span>
        <span class="r">{speed(part.speed)}</span>
      </div>
    {:else}
      <div class="none">
        {#if item?.status === 'done'}
          All parts arrived. The file is complete.
        {:else if item?.status === 'paused'}
          Paused. Every part resumes exactly where it stopped.
        {:else if item?.status === 'queued'}
          Waiting for a free slot. Parts appear as soon as it starts.
        {:else if item?.status === 'downloading' && p && !p.parts.length && p.size == null}
          The server did not say how big the file is, so it loads on one line.
        {:else}
          Connecting…
        {/if}
      </div>
    {/each}
  </div>
</section>

<style>
  .parts {
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
    grid-template-columns: minmax(0, 1fr) repeat(4, auto) auto;
    gap: 26px;
    align-items: end;
    padding-bottom: 14px;
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
    font-size: 34px;
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
  .mapwrap {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding-bottom: 14px;
  }
  .maphead {
    display: flex;
    justify-content: space-between;
    gap: 16px;
  }
  .legend {
    display: flex;
    align-items: center;
    gap: 6px;
    text-transform: none;
    letter-spacing: 0.02em;
  }
  .k {
    width: 10px;
    height: 10px;
    border-radius: 2px;
    display: inline-block;
    margin-left: 10px;
  }
  .k.loaded {
    background: rgb(242 242 238 / 0.82);
  }
  .k.miss {
    background: #15191e;
    box-shadow: inset 0 0 0 1px var(--line-strong);
  }
  .k.head {
    width: 3px;
    background: var(--alert);
  }
  .explain {
    color: var(--fog);
    max-width: 75ch;
  }
  .cols,
  .row {
    display: grid;
    grid-template-columns: 64px minmax(170px, 0.8fr) minmax(120px, 1.2fr) 60px 110px;
    column-gap: 16px;
    align-items: center;
    padding: 0 10px;
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
    height: 38px;
    border-bottom: 1px solid var(--line);
    font-family: var(--mono);
    font-stretch: 75%;
    font-size: 14px;
    transition: background 600ms var(--ease), box-shadow 600ms var(--ease);
  }
  .row.new {
    background: rgb(255 212 0 / 0.1);
    box-shadow: inset 0 0 0 1px rgb(255 212 0 / 0.45);
  }
  .no {
    font-family: var(--gothic);
    font-weight: 600;
    font-size: 20px;
  }
  .range {
    display: flex;
    gap: 10px;
    align-items: center;
    white-space: nowrap;
    overflow: hidden;
  }
  .split {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    color: var(--alert-ink);
    background: var(--alert);
    padding: 2px 6px;
    border-radius: 3px;
  }
  .split {
    flex: none; /* the source part number is the badge's whole message: never clip it */
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .rt {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .split .short {
    display: none;
  }
  .track {
    position: relative;
    height: 8px;
    border-radius: 2px;
    background: #15191e;
    box-shadow: inset 0 0 0 1px var(--line);
    overflow: hidden;
  }
  .fill {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    background: rgb(242 242 238 / 0.82);
    border-right: 2px solid var(--alert);
  }
  .pc {
    color: var(--fog);
  }
  .none {
    padding: 24px 10px;
    color: var(--fog);
  }
  @container (max-width: 760px) {
    header {
      grid-template-columns: minmax(0, 1fr) auto auto auto;
      gap: 16px;
    }
    /* Narrow: drop "Parts so far" first; speed is the product. */
    header .fig:nth-of-type(3) {
      display: none;
    }
    .big,
    h2 {
      font-size: 28px;
    }
    .legend {
      display: none;
    }
    .split .long {
      display: none;
    }
    /* A freshly split row says where it came from; its range returns when the light fades. */
    .row.new .rt {
      display: none;
    }
    .split .short {
      display: inline-flex;
    }
    .cols,
    .row {
      grid-template-columns: 52px minmax(130px, 0.9fr) minmax(80px, 1fr) 48px 92px;
      column-gap: 10px;
    }
  }
</style>
