<script lang="ts">
  // A download's bar cut into its pieces: one cell per part a connection is fetching, each
  // filling on its own from where it starts, with a yellow head where it is writing. Finished
  // stretches are solid, untouched ones dark. Torrents show their pieces folded into cells.
  import type { Progress } from './types';

  let {
    p,
    fraction = 0,
    tone = 'ivory',
    height = 10,
    cells = 24,
  }: {
    p?: Progress;
    /** Used when there are no parts (a paused, queued or video download). */
    fraction?: number;
    tone?: 'ivory' | 'steel' | 'ok';
    height?: number;
    /** How many cells a torrent's piece map is folded into. */
    cells?: number;
  } = $props();

  interface Cell {
    /** Left edge and width, 0..1 of the file. */
    at: number;
    w: number;
    /** How much of the cell is on disk, from its left edge. */
    fill: number;
    /** A live part: where its connection writes (0..1 of the cell). */
    head: number | null;
  }

  const clamp = (x: number) => Math.min(1, Math.max(0, x));

  const list = $derived.by<Cell[]>(() => {
    const size = p?.size || 0;
    // Torrent: the piece map folded into equal cells.
    const pieces = p?.torrent?.pieces;
    if (pieces?.length) {
      const n = Math.min(cells, pieces.length);
      const out: Cell[] = [];
      for (let i = 0; i < n; i++) {
        const a = Math.floor((i * pieces.length) / n);
        const b = Math.max(a + 1, Math.floor(((i + 1) * pieces.length) / n));
        const slice = pieces.slice(a, b);
        out.push({ at: i / n, w: 1 / n, fill: clamp(slice.reduce((x, y) => x + y, 0) / slice.length), head: null });
      }
      return out;
    }
    // A plain file with live parts: cells follow the parts, gaps fill the stretches between.
    if (size && p?.parts?.length) {
      const parts = [...p.parts].sort((a, b) => a.start - b.start);
      const gaps = p.gaps ?? [];
      const out: Cell[] = [];
      // A stretch no connection is on: solid where it is on disk, dark where it is not.
      const between = (from: number, to: number) => {
        if (to - from <= 0) return;
        let cur = from;
        const runs = gaps.filter(([a, b]) => b > from && a < to).sort((x, y) => x[0] - y[0]);
        for (const [a, b] of runs) {
          const s = Math.max(a, from);
          const e = Math.min(b, to);
          if (s > cur) out.push({ at: cur, w: s - cur, fill: 1, head: null });
          out.push({ at: s, w: e - s, fill: 0, head: null });
          cur = e;
        }
        if (cur < to) out.push({ at: cur, w: to - cur, fill: 1, head: null });
      };
      let cur = 0;
      for (const part of parts) {
        const a = part.start / size;
        const b = part.end / size;
        between(cur, a);
        const span = Math.max(1, part.end - part.start);
        const fill = clamp((part.pos - part.start) / span);
        out.push({ at: a, w: b - a, fill, head: fill < 1 ? fill : null });
        cur = Math.max(cur, b);
      }
      between(cur, 1);
      return out;
    }
    return [{ at: 0, w: 1, fill: clamp(fraction), head: null }];
  });
</script>

<div class="bar {tone}" style:height="{height}px" aria-hidden="true">
  {#each list as c, i (i)}
    <!-- A seam between cells, left off cells too thin to carry one. -->
    {@const seam = i > 0 && c.w > 0.015}
    <span class="cell" style:left="calc({c.at * 100}% + {seam ? 1 : 0}px)" style:width="calc({c.w * 100}% - {seam ? 1 : 0}px)">
      <span class="fill" style:width="{c.fill * 100}%"></span>
      {#if c.head !== null}<span class="head" style:left="{c.head * 100}%"></span>{/if}
    </span>
  {/each}
</div>

<style>
  .bar {
    position: relative;
    width: 100%;
    border-radius: 2px;
    overflow: hidden;
    contain: layout paint;
  }
  /* Each cell carries its own dark ground; the 1px between cells shows the row behind, so
     the pieces read as pieces on any row colour. */
  .cell {
    position: absolute;
    top: 0;
    bottom: 0;
    overflow: hidden;
    background: rgb(255 255 255 / 0.08);
  }
  .fill {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    background: rgb(242 242 238 / 0.82);
    transition: width 0.4s ease-out;
  }
  .steel .fill {
    background: var(--steel);
  }
  .ok .fill {
    background: var(--ok);
  }
  .head {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    margin-left: -1px;
    background: var(--alert);
    transition: left 0.4s ease-out;
  }
  .steel .head {
    display: none;
  }
  @media (prefers-reduced-motion: reduce) {
    .fill,
    .head {
      transition: none;
    }
  }
</style>
