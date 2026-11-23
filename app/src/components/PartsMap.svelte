<script lang="ts">
  // The whole file as one bar. Loaded bytes are filled; each live part is a framed band
  // (one connection) with a head where it is writing. A split drops a cut and flashes yellow.
  import { app } from '../lib/state.svelte';
  import { box, glide, flash, cut } from '../lib/motion';
  import type { Progress } from '../lib/types';

  let {
    p,
    fallback = 0,
    tone = 'glass',
    height = 40,
    labels = false,
  }: { p?: Progress; fallback?: number; tone?: 'glass' | 'paper'; height?: number; labels?: boolean } = $props();

  const size = $derived(p?.size || 0);
  const meta = $derived(p ? app.partMeta[p.id] : undefined);
  const holes = $derived<[number, number][]>(p?.gaps?.length ? p.gaps : [[Math.min(1, Math.max(0, fallback)), 1]]);
  const parts = $derived(size && p ? [...p.parts].sort((a, b) => a.start - b.start) : []);
  const pct = (x: number) => (size ? (x / size) * 100 : 0);
</script>

<div class="map {tone}" style:height="{height}px" role="img" aria-label="File map: {parts.length} parts downloading at once">
  <div class="loaded"></div>
  {#each holes as [a, b], i (i)}
    <div class="hole" use:box={{ left: a * 100, width: Math.max(0, (b - a) * 100) }}></div>
  {/each}
  {#each parts as part (part.start)}
    {@const n = meta?.nums[part.start]}
    {@const isNew = meta?.fresh[part.start] !== undefined}
    {@const w = pct(part.end - part.start)}
    <div class="band" class:new={isNew} use:box={{ left: pct(part.start), width: w }}>
      <div class="head" use:glide={{ prop: 'left', value: ((part.pos - part.start) / Math.max(1, part.end - part.start)) * 100 }}></div>
      {#if labels && n && w > 3.2}
        <span class="num">{String(n).padStart(2, '0')}</span>
      {/if}
      {#if isNew}
        <div class="flash" use:flash></div>
        <div class="cut" use:cut></div>
      {/if}
    </div>
  {/each}
</div>

<style>
  .map {
    position: relative;
    width: 100%;
    border-radius: 4px;
    overflow: hidden;
    contain: layout paint;
  }
  .loaded,
  .hole,
  .band {
    position: absolute;
    top: 0;
    bottom: 0;
  }
  .loaded {
    left: 0;
    right: 0;
  }
  .band {
    box-sizing: border-box;
  }
  .head {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    margin-left: -1px;
    background: var(--alert);
  }
  .num {
    position: absolute;
    left: 4px;
    top: 3px;
    font-family: var(--mono);
    font-stretch: 75%;
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.04em;
    pointer-events: none;
  }
  .flash {
    position: absolute;
    inset: 0;
    background: var(--alert);
  }
  .cut {
    position: absolute;
    left: -1px;
    top: -2px;
    bottom: -2px;
    width: 2px;
    background: var(--alert);
    transform-origin: top;
  }

  /* On glass: loaded bytes ivory, missing bytes smoked, each live part framed. */
  .glass {
    background: rgb(255 255 255 / 0.05);
  }
  .glass .loaded {
    background: rgb(242 242 238 / 0.82);
  }
  .glass .hole {
    background: #15191e;
  }
  .glass .band {
    border: 1px solid rgb(242 242 238 / 0.32);
    border-left-color: rgb(242 242 238 / 0.75);
  }
  .glass .num {
    color: var(--fog);
  }

  /* On the ivory pass: loaded bytes print as the scannable block. */
  .paper .loaded {
    background: repeating-linear-gradient(
      90deg,
      #15181c 0 2px,
      transparent 2px 3px,
      #15181c 3px 4px,
      transparent 4px 6px,
      #15181c 6px 9px,
      transparent 9px 10px
    );
  }
  .paper .hole {
    background: #d6d6cf;
  }
  .paper .band {
    border: 1px solid rgb(21 24 28 / 0.35);
    border-left: 1px solid #15181c;
  }
  .paper .head {
    background: #c9a300;
    width: 3px;
  }
  .paper .num {
    color: #565d65;
  }
  .band.new {
    border-color: var(--alert);
  }
</style>
