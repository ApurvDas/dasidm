<script lang="ts">
  import { api } from '../lib/api';
  import { app, run } from '../lib/state.svelte';
  import { bytes, doneAt, duration, host, percent, speed, when } from '../lib/format';
  import Icon from '../lib/Icon.svelte';
  import { CATEGORIES, category, typeIcon } from '../lib/types';
  import PartsMap from './PartsMap.svelte';
  import { thumbOf } from '../lib/state.svelte';
  import { PHASE, mediaLine } from '../lib/media';
  import { clock } from '../lib/format';

  const item = $derived(app.selected ? app.items[app.selected] : null);
  const p = $derived(item ? app.progress[item.id] : undefined);
  const size = $derived(p?.size ?? item?.size ?? null);
  const loaded = $derived(p?.downloaded ?? item?.downloaded ?? 0);
  const pc = $derived(percent(loaded, size));
  const seq = $derived(item ? String(Object.values(app.items).filter((i) => i.addedAt <= item.addedAt).length).padStart(4, '0') : '');
  const catName = $derived(CATEGORIES.find((c) => c.id === item?.category)?.name ?? 'Other');
  const ext = $derived(item?.fileName.match(/\.([a-z0-9]{1,6})$/i)?.[1].toUpperCase() ?? '');


  /** Short source code for the route line, like an airport code: releases.ubuntu.com -> UBUNTU. */
  function sourceCode(url: string) {
    const h = host(url);
    if (/^[\d.]+$/.test(h) || h.includes(':') || !h.includes('.')) return h;
    const parts = h.split('.');
    const tld = parts[parts.length - 1];
    const sld = parts[parts.length - 2];
    // example.co.uk / example.com.au: skip the short second-level label
    const name = parts.length > 2 && tld.length === 2 && sld.length <= 3 ? parts[parts.length - 3] : sld;
    return name;
  }

  async function act(fn: () => Promise<unknown>) {
    await run(fn());
  }
</script>

<aside class="pass-col" aria-label="Selected download">
  {#if item}
    <article class="pass" class:failed={item.status === 'failed'}>
      {#if item.media}
        {@const art = thumbOf(item)}
        {#if art}
          <!-- Real media art is the only imagery DasIDM shows. -->
          <img class="art" src={art} alt="" />
        {/if}
      {/if}
      <header>
        <div>
          <h3 title={item.fileName}>{item.fileName.replace(/([_.\-])/g, '$1​')}</h3>
          <span class="kind"
            ><Icon name={typeIcon(item.category)} size={14} />{category(item.category).short}{#if item.media && !item.media.audioOnly}<span class="q"
                >{item.media.quality}</span
              >{/if}{#if ext}<span class="ext">{ext}</span>{/if}{#if item.media?.duration}<span class="len">{clock(item.media.duration)}</span>{/if}</span
          >
          {#if item.media?.channel}<span class="by">{item.media.channel}</span>{/if}
        </div>
        <div class="seq">
          <span class="lbl">Seq</span>
          <b>{seq}</b>
        </div>
      </header>

      <!-- The route line: where it comes from and where it lands, like SFO → ORD. -->
      <div class="route">
        <div class="end">
          <b class="code" title={item.url}>{sourceCode(item.url)}</b>
          <span class="lbl full" title={item.url}>From {host(item.url)}</span>
        </div>
        <span class="arrow" aria-hidden="true"><Icon name="right" size={18} /></span>
        <div class="end to">
          <b class="code" title={item.saveDir}>{item.saveDir.split('\\').pop() || item.saveDir}</b>
          <span class="lbl full" title={item.saveDir}>To {item.saveDir}</span>
        </div>
      </div>
      <div class="seg two">
        <div><span class="lbl">Loaded</span><b>{bytes(loaded)}</b><span class="sub">of {bytes(size)}{pc != null ? ` · ${pc}%` : ''}</span></div>
        <div>
          {#if item.status === 'done'}
            <span class="lbl">Arrived</span><b>{when(item.finishedAt)}</b>
          {:else if item.status === 'downloading'}
            <span class="lbl">Done at</span><b>{doneAt(p?.eta)}</b><span class="sub">{duration(p?.eta)} left</span>
          {:else}
            <span class="lbl">Status</span><b>{item.status === 'queued' ? 'Waiting' : item.status === 'paused' ? 'Paused' : 'Failed'}</b>
          {/if}
        </div>
      </div>
      <div class="seg three">
        <div>
          <span class="lbl">Speed</span>
          {#if item.status === 'downloading' && p?.speed}
            {@const [n, u] = speed(p.speed).split(' ')}
            <b>{n}<span class="sub unit">{u}</span></b>
          {:else}
            <b>—</b>
          {/if}
        </div>
        {#if item.media}
          <div><span class="lbl">Quality</span><b>{item.media.audioOnly ? item.media.format : item.media.quality}</b></div>
        {:else}
          <div><span class="lbl">Lines</span><b>{item.status === 'downloading' ? (p?.activeConnections ?? 0) : '—'}<span class="sub"> / {item.connections}</span></b></div>
        {/if}
        <div><span class="lbl">Resume</span><b>{item.resumable || item.status === 'done' ? 'Yes' : 'No'}</b></div>
      </div>

      <div class="scan">
        {#if item.status === 'done'}
          <div class="complete" aria-label="All parts on disk"></div>
        {:else}
          <PartsMap {p} fallback={size ? loaded / size : 0} tone="paper" height={56} />
        {/if}
        <div class="scanfoot">
          <span class="lbl">
            {#if item.status === 'done'}
              {catName} · all parts arrived
            {:else if item.status === 'downloading' && p?.phase}
              {catName} · {PHASE[p.phase]}
            {:else if item.status === 'downloading' && p}
              {catName} · {p.partsTotal} {p.partsTotal === 1 ? 'part' : 'parts'} · {p.parts.length} loading now
            {:else}
              {catName} · {pc ?? 0}% on disk
            {/if}
          </span>
          {#if item.status === 'downloading' && item.kind !== 'video'}
            <button class="partsbtn" onclick={() => (app.partsOpen = true)}>Open parts<kbd>P</kbd></button>
          {/if}
        </div>
      </div>

      {#if item.status === 'failed' && item.error}
        <p class="err"><Icon name="alert" /> {item.error}</p>
      {/if}
    </article>

    <div class="actions">
      {#if item.status === 'downloading' || item.status === 'queued'}
        <button class="btn" onclick={() => act(() => api.pause([item.id]))}><Icon name="pause" />Pause<kbd>Space</kbd></button>
      {:else if item.status === 'paused' || item.status === 'failed'}
        <button class="btn primary" onclick={() => act(() => api.resume([item.id]))}><Icon name="play" />{item.status === 'failed' ? 'Retry' : 'Resume'}<kbd>Space</kbd></button>
        <button class="btn" onclick={() => act(() => api.restart([item.id]))}><Icon name="restart" />Start over</button>
      {/if}
      {#if item.status === 'done'}
        <button class="btn primary" onclick={() => act(() => api.openFile(item.id))}><Icon name="open" />Open<kbd>Enter</kbd></button>
      {/if}
      <button class="btn" onclick={() => act(() => api.openFolder(item.id))}><Icon name="folder" />Show in folder</button>
      <button class="btn ghost danger" onclick={() => (app.confirmDelete = true)}><Icon name="trash" />Remove<kbd>Del</kbd></button>
    </div>

    {#if app.confirmDelete}
      <div class="confirm" role="alertdialog" aria-label="Remove download">
        <p>
          {#if item.status === 'done'}Remove from the list, or delete the file from disk too?{:else}Remove this download? The partly downloaded data is deleted.{/if}
        </p>
        <div class="row">
          <button class="btn" onclick={() => act(() => api.remove([item.id], false))}>Remove from list</button>
          {#if item.status === 'done'}
            <button class="btn danger" onclick={() => act(() => api.remove([item.id], true))}>Delete file too</button>
          {/if}
          <button class="btn ghost" onclick={() => (app.confirmDelete = false)}>Keep</button>
        </div>
      </div>
    {/if}
  {:else}
    <div class="nothing">
      <p class="t">No download selected</p>
      <p>Pick a row to see its pass: where it comes from, where it lands, and a live map of every connection.</p>
      <dl>
        <dt><kbd>Ctrl V</kbd></dt><dd>Add a copied link</dd>
        <dt><kbd>Space</kbd></dt><dd>Pause or resume</dd>
        <dt><kbd>Enter</kbd></dt><dd>Open a finished file</dd>
        <dt><kbd>Del</kbd></dt><dd>Remove</dd>
        <dt><kbd>↑ ↓</kbd></dt><dd>Move through the list</dd>
      </dl>
    </div>
  {/if}
</aside>

<style>
  .pass-col {
    background: var(--glass-pass);
    box-shadow: var(--edge);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
    overflow: auto;
  }
  .route {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    align-items: center;
    gap: 10px;
    padding: 12px 16px;
    border-top: 1px dashed #a7aca4;
  }
  .route .end {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .route .to {
    text-align: right;
  }
  .route .code {
    font-family: var(--gothic);
    font-weight: 700;
    font-size: 24px;
    line-height: 1.05;
    text-transform: uppercase;
    letter-spacing: 0.02em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .route .arrow {
    color: #15181c;
  }
  .route .full {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    text-transform: none;
    letter-spacing: 0.02em;
  }
  .pass {
    flex: none;
    background: var(--ivory);
    color: #15181c;
    border-radius: 12px;
    overflow: hidden;
    box-shadow: 0 10px 30px rgb(0 0 0 / 0.35);
  }
  header {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 12px;
    padding: 14px 16px;
    background: #e4e4dd;
  }
  h3 {
    font-family: var(--mono);
    font-stretch: 75%;
    font-weight: 700;
    font-size: 17px;
    line-height: 1.25;
    overflow-wrap: anywhere;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    margin-top: 2px;
  }
  .seq {
    text-align: right;
  }
  .seq b,
  .seg b {
    display: block;
    font-family: var(--mono);
    font-stretch: 75%;
    font-weight: 600;
    font-size: 17px;
    margin-top: 2px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .art {
    display: block;
    width: 100%;
    aspect-ratio: 16 / 9;
    max-height: 168px;
    object-fit: cover;
    background: #2a2f36;
  }
  .q,
  .len {
    font-family: var(--mono);
    font-stretch: 75%;
    font-size: 11px;
    font-weight: 600;
  }
  .by {
    display: block;
    margin-top: 2px;
    font-size: 12px;
    color: #565d65;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .kind {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 6px;
    font-size: 12px;
    font-weight: 600;
    color: #3b4148;
  }
  .kind :global(svg) {
    flex: none;
  }
  .ext {
    font-family: var(--mono);
    font-stretch: 75%;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.06em;
    padding: 1px 5px;
    border-radius: 3px;
    border: 1px solid #b9b9b1;
    color: #565d65;
  }
  .lbl {
    font-family: var(--mono);
    font-stretch: 75%;
    font-size: 10px;
    font-weight: 500;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #565d65;
  }
  .sub {
    font-family: var(--ui);
    font-size: 12px;
    color: #565d65;
    font-weight: 400;
  }
  .seg {
    display: grid;
    border-top: 1px dashed #a7aca4;
  }
  .seg.two {
    grid-template-columns: 1fr 1fr;
  }
  .seg.three {
    grid-template-columns: 1.6fr 0.9fr 0.9fr;
  }
  .seg.three b {
    font-size: 15px;
  }
  .seg > div {
    padding: 10px 16px;
    min-width: 0;
  }
  .seg > div + div {
    border-left: 1px dashed #a7aca4;
  }
  .scan {
    border-top: 1px dashed #a7aca4;
    padding: 12px 16px 14px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .complete {
    height: 56px;
    background: repeating-linear-gradient(90deg, #15181c 0 2px, transparent 2px 3px, #15181c 3px 4px, transparent 4px 6px, #15181c 6px 9px, transparent 9px 10px);
  }
  .scanfoot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .partsbtn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 10px;
    border-radius: 5px;
    background: #15181c;
    color: var(--ivory);
    font-weight: 600;
    font-size: 12px;
    white-space: nowrap;
  }
  .partsbtn:hover {
    background: #2a3038;
  }
  .unit {
    margin-left: 4px;
  }
  .err {
    display: flex;
    gap: 8px;
    align-items: center;
    padding: 10px 16px;
    background: #ffe3e3;
    color: #8a1c1c;
    font-weight: 600;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
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
  .confirm {
    border: 1px solid rgb(255 107 107 / 0.4);
    background: rgb(255 107 107 / 0.08);
    border-radius: var(--radius);
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .confirm .row {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .nothing {
    color: var(--fog);
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 6px 4px;
  }
  .nothing .t {
    color: var(--ivory);
    font-weight: 600;
    font-size: 15px;
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 8px 12px;
    margin-top: 8px;
    align-items: center;
  }
  dd {
    color: var(--ivory);
  }
  .nothing kbd {
    opacity: 1;
    border-color: var(--line-strong);
    color: var(--fog);
  }
</style>
