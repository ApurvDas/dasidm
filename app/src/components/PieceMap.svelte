<script lang="ts">
  // A torrent's pieces in order: each bar is one stretch of the torrent, filled as far as its
  // pieces are verified on disk. Torrents arrive out of order, so this is a scatter, not a bar.
  let {
    pieces,
    fallback = 0,
    height = 56,
    tone = 'board',
  }: { pieces: number[] | null | undefined; fallback?: number; height?: number; tone?: 'paper' | 'board' } = $props();

  const bars = $derived(pieces?.length ? pieces : Array.from({ length: 96 }, (_, i) => (i / 96 < fallback ? 1 : 0)));
  const done = $derived(bars.length ? bars.reduce((a, b) => a + b, 0) / bars.length : 0);
</script>

<div
  class="map {tone}"
  style:height="{height}px"
  role="img"
  aria-label="Pieces on disk: {Math.round(done * 100)}%"
>
  {#each bars as f, i (i)}
    <span class="bar" class:full={f >= 1} class:empty={f <= 0} style:--f={f}></span>
  {/each}
</div>

<style>
  .map {
    display: flex;
    gap: 1px;
    width: 100%;
    border-radius: 4px;
    overflow: hidden;
  }
  .bar {
    flex: 1;
    min-width: 0;
    position: relative;
  }
  /* A partly verified stretch fills from the bottom, like a level. */
  .bar::after {
    content: '';
    position: absolute;
    inset: auto 0 0 0;
    height: calc(var(--f) * 100%);
  }
  .paper {
    background: transparent;
  }
  .paper .bar {
    background: #d9d9d2;
  }
  .paper .bar::after {
    background: #15181c;
  }
  .board .bar {
    background: #15191e;
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.04);
  }
  .board .bar::after {
    background: rgb(242 242 238 / 0.82);
  }
</style>
