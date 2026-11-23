<script lang="ts">
  // The quality picker for video and music links: a resolution, or a sound-only format.
  import { bytes, clock } from '../lib/format';
  import Icon from '../lib/Icon.svelte';
  import type { VideoInfo, VideoOption } from '../lib/types';

  let {
    info,
    chosen = $bindable(),
    playlist = $bindable(false),
  }: { info: VideoInfo; chosen: VideoOption | null; playlist: boolean } = $props();

  const videoOpts = $derived(info.options.filter((o) => o.choice.kind === 'video'));
  const audioOpts = $derived(info.options.filter((o) => o.choice.kind === 'audio'));
  // Music sites start on sound; everything else on video.
  let tab = $state<'video' | 'audio'>(/music\.|soundcloud|bandcamp/i.test(info.url) ? 'audio' : 'video');
  const shown = $derived(tab === 'video' && videoOpts.length ? videoOpts : audioOpts);

  $effect(() => {
    if (!videoOpts.length) tab = 'audio';
  });
  // Changing tab picks that tab's suggestion; the choice stays when the list re-renders.
  $effect(() => {
    if (!chosen || !shown.includes(chosen)) chosen = shown.find((o) => o.recommended) ?? shown[0] ?? null;
  });
  $effect(() => {
    if (info.playlistOnly) playlist = true;
  });

  const meta = $derived([info.channel, clock(info.duration), info.site].filter(Boolean).join(' · '));
</script>

<div class="picker">
  <div class="head">
    <Icon name={tab === 'audio' ? 'type-music' : 'type-video'} size={18} />
    <div class="title">
      <b title={info.title}>{info.title}</b>
      {#if meta}<span class="meta">{meta}</span>{/if}
    </div>
  </div>

  {#if info.isLive}
    <p class="warn"><Icon name="alert" /> This is a live stream. DasIDM records it from now until it ends or you pause.</p>
  {/if}

  <div class="bar">
    <div class="seg" role="tablist" aria-label="What to keep">
      {#if videoOpts.length}
        <button role="tab" aria-selected={tab === 'video'} class:on={tab === 'video'} onclick={() => (tab = 'video')}><Icon name="type-video" size={14} />Video</button>
      {/if}
      <button role="tab" aria-selected={tab === 'audio'} class:on={tab === 'audio'} onclick={() => (tab = 'audio')}><Icon name="type-music" size={14} />Sound only</button>
    </div>
    {#if info.playlist}
      <div class="seg" role="radiogroup" aria-label="How much">
        {#if !info.playlistOnly}
          <button role="radio" aria-checked={!playlist} class:on={!playlist} onclick={() => (playlist = false)}>This video</button>
        {/if}
        <button role="radio" aria-checked={playlist} class:on={playlist} onclick={() => (playlist = true)}>
          Whole playlist <span class="n">{info.playlist.count}</span>
        </button>
      </div>
    {/if}
  </div>

  <div class="list" role="radiogroup" aria-label="Quality">
    {#each shown as o (o.label + o.detail)}
      <label class="opt" class:on={chosen === o}>
        <input type="radio" name="quality" checked={chosen === o} onchange={() => (chosen = o)} />
        <span class="dot" aria-hidden="true"></span>
        <b class="q">{o.label}</b>
        <span class="d">{o.detail}{#if o.recommended}<span class="sugg">Suggested</span>{/if}</span>
        <span class="s">{playlist ? '' : o.size ? `${o.approx ? '≈ ' : ''}${bytes(o.size)}` : ''}</span>
      </label>
    {/each}
  </div>

  {#if chosen?.note}
    <p class="note"><Icon name="alert" /> {chosen.note}</p>
  {:else if playlist && info.playlist}
    <p class="note">Each of the {info.playlist.count} videos is added as its own download, at this quality or the nearest below it.</p>
  {/if}
</div>

<style>
  .picker {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 0;
  }
  .head {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    color: var(--fog);
  }
  .head :global(svg) {
    flex: none;
    margin-top: 2px;
  }
  .title {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .title b {
    color: var(--ivory);
    font-size: 15px;
    font-weight: 600;
    line-height: 1.3;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .meta {
    font-size: 12px;
  }
  .bar {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    flex-wrap: wrap;
  }
  .seg {
    display: flex;
    gap: 2px;
    padding: 2px;
    border-radius: 8px;
    background: rgb(255 255 255 / 0.04);
    border: 1px solid var(--line);
  }
  .seg button {
    height: 26px;
    padding: 0 10px;
    border-radius: 6px;
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--fog);
    font-weight: 600;
  }
  .seg button:hover {
    color: var(--ivory);
  }
  .seg button.on {
    color: var(--ivory);
    background: rgb(255 255 255 / 0.1);
  }
  .n {
    font-family: var(--mono);
    font-stretch: 75%;
    font-size: 11px;
    color: var(--fog);
  }
  .list {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--line);
    border-radius: 8px;
    overflow: auto;
    max-height: min(300px, 42vh);
  }
  .opt {
    display: grid;
    grid-template-columns: 14px 58px minmax(0, 1fr) auto;
    column-gap: 10px;
    align-items: center;
    min-height: 34px;
    padding: 0 12px;
    border-bottom: 1px solid var(--line);
    cursor: pointer;
    color: var(--fog);
  }
  .opt:last-child {
    border-bottom: 0;
  }
  .opt:hover {
    background: var(--hover);
  }
  .opt.on {
    background: var(--selected);
    box-shadow: var(--selected-line);
    color: var(--ivory);
  }
  .opt input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
  .opt:has(input:focus-visible) {
    outline: 2px solid var(--ivory);
    outline-offset: -2px;
  }
  .dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    border: 1.5px solid var(--steel);
  }
  .opt.on .dot {
    border-color: var(--ivory);
    background: radial-gradient(circle, var(--ivory) 0 3px, transparent 3.5px);
  }
  .q {
    font-family: var(--mono);
    font-stretch: 75%;
    font-size: 14px;
    font-weight: 700;
    color: var(--ivory);
  }
  .d {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12.5px;
  }
  .sugg {
    margin-left: 8px;
    font-size: 11px;
    padding: 1px 6px;
    border-radius: 8px;
    border: 1px solid var(--line-strong);
    color: var(--ivory);
  }
  .s {
    font-family: var(--mono);
    font-stretch: 75%;
    font-size: 12px;
    color: var(--fog);
    white-space: nowrap;
  }
  .note,
  .warn {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    font-size: 12.5px;
    color: var(--fog);
    line-height: 1.4;
  }
  .note :global(svg),
  .warn :global(svg) {
    flex: none;
    margin-top: 1px;
  }
</style>
