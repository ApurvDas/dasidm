<script lang="ts">
  // Thin live map of a file: filled where bytes are on disk, gaps where they are missing,
  // ticks where each connection is writing right now.
  let {
    gaps = null,
    heads = [],
    fraction = 0,
    tone = 'ivory',
    height = 3,
  }: { gaps?: [number, number][] | null; heads?: number[]; fraction?: number; tone?: string; height?: number } = $props();

  const holes = $derived(gaps ?? [[Math.min(1, Math.max(0, fraction)), 1] as [number, number]]);
</script>

<svg class="strip" viewBox="0 0 1000 10" preserveAspectRatio="none" style:height="{height}px" aria-hidden="true">
  <rect x="0" y="0" width="1000" height="10" class="fill {tone}" />
  {#each holes as [a, b]}
    <rect x={a * 1000} y="0" width={Math.max(0, (b - a) * 1000)} height="10" class="hole" />
  {/each}
  {#each heads as h}
    <rect x={h * 1000 - 2} y="0" width="4" height="10" class="head" />
  {/each}
</svg>

<style>
  .strip {
    display: block;
    width: 100%;
    border-radius: 2px;
    overflow: hidden;
  }
  .fill.ivory {
    fill: rgb(242 242 238 / 0.78);
  }
  .fill.ok {
    fill: var(--ok);
  }
  .fill.steel {
    fill: var(--steel);
  }
  .fill.bad {
    fill: var(--bad);
  }
  .hole {
    fill: rgb(255 255 255 / 0.08);
  }
  .head {
    fill: var(--alert);
  }
</style>
