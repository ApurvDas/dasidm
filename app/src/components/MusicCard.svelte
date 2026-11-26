<script lang="ts">
  // A Spotify / Amazon Music share link: what it is, and where to buy it as a download in the
  // quality you want. DasIDM never downloads from the streaming service (DRM-protected audio).
  import { api } from '../lib/api';
  import { run, toast } from '../lib/state.svelte';
  import { duration as fmtDuration } from '../lib/format';
  import Icon from '../lib/Icon.svelte';
  import type { MusicInfo, MusicStore } from '../lib/types';

  let { url }: { url: string } = $props();

  let info = $state<MusicInfo | null>(null);
  let loading = $state(true);
  let stores = $state<MusicStore[]>([]);
  let artist = $state('');
  let title = $state('');

  $effect(() => {
    api.musicStores().then((s) => (stores = s)).catch(() => {});
  });
  $effect(() => {
    const u = url;
    let live = true;
    loading = true;
    info = null;
    api
      .musicLookup(u)
      .then((r) => {
        if (!live) return;
        info = r;
        artist = r.artist ?? '';
        title = (r.kind === 'album' ? r.album : r.title) ?? r.title ?? '';
      })
      .catch((e) => {
        if (live) info = { service: null, kind: null, title: null, artist: null, album: null, year: null, duration: null, tracks: null, cover: null, manual: true, note: String(e) };
      })
      .finally(() => {
        if (live) loading = false;
      });
    return () => (live = false);
  });

  const service = $derived(info?.service === 'amazonMusic' || /amazon\./i.test(url) ? 'Amazon Music' : 'Spotify');
  const query = $derived([artist.trim(), title.trim()].filter(Boolean).join(' '));
  const what = $derived(info?.kind === 'album' ? 'album' : info?.kind === 'playlist' ? 'playlist' : info?.kind === 'artist' ? 'artist' : 'track');
  const meta = $derived(
    [
      info?.kind === 'album' ? 'Album' : info?.kind === 'playlist' ? 'Playlist' : info?.kind === 'artist' ? 'Artist' : 'Track',
      info?.artist && info.kind !== 'artist' ? info.artist : null,
      info?.kind === 'track' && info.album ? info.album : null,
      info?.year ? String(info.year) : null,
      info?.duration ? fmtDuration(info.duration) : null,
      info?.tracks ? `${info.tracks} tracks` : null,
    ].filter(Boolean),
  );

  function search(id: string) {
    if (query) run(api.openStoreSearch(id, query));
  }
  async function copy() {
    const text = [artist.trim(), title.trim()].filter(Boolean).join(' – ');
    await navigator.clipboard.writeText(text).then(() => toast(`Copied “${text}”`)).catch(() => {});
  }
</script>

<div class="music" aria-live="polite">
  {#if loading}
    <span class="cap">Reading the {service} link…</span>
  {:else if info}
    {#if !info.manual}
      <div class="head">
        {#if info.cover}<img class="cover" src={info.cover} alt="" />{:else}<div class="cover none"><Icon name="type-music" /></div>{/if}
        <div class="title">
          <b>{info.title}</b>
          <span class="meta">{meta.join(' · ')}</span>
        </div>
      </div>
    {/if}
    <p class="why">
      <Icon name="alert" />
      <span>{service} only streams, with DRM, so DasIDM can't download from it. Buy the {what} where you can choose the quality, then download it here.</span>
    </p>
    {#if info.manual && info.note}<span class="note">{info.note}</span>{/if}
    {#if info.manual || info.kind === 'playlist'}
      <div class="two">
        <label><span class="cap">Artist</span><input class="field" bind:value={artist} spellcheck="false" placeholder="Artist" /></label>
        <label><span class="cap">{info.kind === 'album' ? 'Album' : 'Title'}</span><input class="field" bind:value={title} spellcheck="false" placeholder="Title" /></label>
      </div>
      {#if info.kind === 'playlist'}<span class="note">A playlist is searched by its name; paste a track or album link for an exact match.</span>{/if}
    {/if}
    <ul class="stores">
      {#each stores as s (s.id)}
        <li>
          <div class="st"><b>{s.name}</b><span class="note">{s.sells}</span></div>
          <button class="btn" disabled={!query} onclick={() => search(s.id)} title={query ? `Search ${s.name} for “${query}”` : 'Type an artist or title first'}><Icon name="search" />Search</button>
        </li>
      {/each}
    </ul>
    <div class="foot">
      <span class="note">Formats and availability vary by release and country.</span>
      <button class="btn ghost" disabled={!query} onclick={copy}><Icon name="copy" />Copy “Artist – Title”</button>
    </div>
  {/if}
</div>

<style>
  .music {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .head {
    display: flex;
    gap: 12px;
    align-items: center;
  }
  .cover {
    width: 64px;
    height: 64px;
    flex: none;
    border-radius: 6px;
    object-fit: cover;
    border: 1px solid var(--line);
  }
  .cover.none {
    display: grid;
    place-items: center;
    color: var(--fog);
    background: rgb(255 255 255 / 0.04);
  }
  .title {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .title b {
    color: var(--ivory);
    font-size: 15px;
    font-weight: 600;
    line-height: 1.3;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    color: var(--fog);
    font-size: 12px;
  }
  .why {
    display: flex;
    gap: 8px;
    margin: 0;
    color: var(--fog);
    font-size: 12.5px;
    line-height: 1.45;
  }
  .why :global(svg) {
    flex: none;
    margin-top: 2px;
  }
  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }
  .two label {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .stores {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--line);
    border-radius: 8px;
  }
  .stores li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 9px 12px;
  }
  .stores li + li {
    border-top: 1px solid var(--line);
  }
  .st {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }
  .st b {
    color: var(--ivory);
    font-weight: 600;
  }
  .note {
    color: var(--fog);
    font-size: 12px;
    line-height: 1.4;
  }
  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }
</style>
