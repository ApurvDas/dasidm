<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { api } from '../lib/api';
  import { app, run, select } from '../lib/state.svelte';
  import { bytes } from '../lib/format';
  import Icon from '../lib/Icon.svelte';
  import VideoPicker from './VideoPicker.svelte';
  import TorrentPicker from './TorrentPicker.svelte';
  import { isLocalPath, isTorrentLink, isWebPage, type ProbeInfo, type TorrentPreview, type VideoInfo, type VideoOption } from '../lib/types';

  let url = $state(typeof app.adding === 'string' ? app.adding : '');
  let name = $state('');
  let folder = $state('');
  let lines = $state(app.settings?.connections ?? 16);
  let info = $state<ProbeInfo | null>(null);
  let probing = $state(false);
  let error = $state('');
  let input = $state<HTMLInputElement>();
  let token = 0;
  /** The link came from the clipboard (Add URL / Ctrl+N), not from typing or pasting. */
  let fromClip = $state(false);

  // Video and music links.
  let video = $state<VideoInfo | null>(null);
  let resolving = $state(false);
  let videoError = $state('');
  let chosen = $state<VideoOption | null>(null);
  let playlist = $state(false);
  let installing = $state(false);

  // Torrents (magnet links, .torrent links and files).
  let tpreview = $state<TorrentPreview | null>(null);
  let tloading = $state(false);
  let terror = $state('');
  let tchosen = $state<Set<number>>(new Set());
  let ttrackers = $state('');

  /** Sites that are video or music pages almost always: read them with yt-dlp right away. */
  const VIDEO_HOSTS =
    /(^|\.)(youtube\.com|youtu\.be|youtube-nocookie\.com|vimeo\.com|soundcloud\.com|bandcamp\.com|dailymotion\.com|twitch\.tv|tiktok\.com|instagram\.com|x\.com|twitter\.com|reddit\.com|facebook\.com|bilibili\.com|rumble\.com|odysee\.com|mixcloud\.com)$/i;
  const videoHost = (u: string) => {
    try {
      return VIDEO_HOSTS.test(new URL(u).hostname);
    } catch {
      return false;
    }
  };

  onMount(async () => {
    if (app.adding !== true) return;
    const link = await api.clipboardLink().catch(() => null);
    if (link && !url) {
      url = link;
      fromClip = true;
      await tick();
      input?.select();
    }
  });

  async function browse() {
    const start = folder.trim() || app.settings?.downloadDir;
    const picked = await run(api.pickFolder(start));
    if (picked) folder = picked;
  }

  $effect(() => {
    input?.focus();
    input?.select();
  });

  const magnet = $derived(/^magnet:\?/i.test(url.trim()));
  const localTorrent = $derived(isLocalPath(url) && /\.torrent$/i.test(url.trim()));
  const valid = $derived(/^https?:\/\/\S+$/i.test(url.trim()) || magnet || localTorrent);
  // A link that answers with HTML is a page (e.g. a share link), not a file.
  const isPage = $derived(!!info && isWebPage(info.content_type));
  const toolsReady = $derived(!!app.tools?.ready);
  /** A torrent: by its link, or because the server says the file is one. */
  const torrentish = $derived(valid && (isTorrentLink(url) || /bittorrent/i.test(info?.content_type ?? '')));
  /** Worth asking yt-dlp: a known video site, or any link that turned out to be a page. */
  const maybeVideo = $derived(valid && !torrentish && (videoHost(url.trim()) || isPage));

  $effect(() => {
    const u = url.trim();
    info = null;
    error = '';
    video = null;
    videoError = '';
    chosen = null;
    playlist = false;
    resolving = false;
    tpreview = null;
    terror = '';
    tloading = false;
    const mine = ++token;
    if (!/^https?:\/\/\S+$/i.test(u)) return;
    probing = !videoHost(u) && !isTorrentLink(u);
    const t = setTimeout(async () => {
      if (videoHost(u) || isTorrentLink(u)) return; // read by yt-dlp / the torrent session instead
      try {
        const r = await api.probe(u);
        if (mine !== token) return;
        info = r;
        if (!name) name = r.file_name;
      } catch (e) {
        if (mine === token) error = String(e);
      } finally {
        if (mine === token) probing = false;
      }
    }, 350);
    return () => clearTimeout(t);
  });

  // Read the page with yt-dlp once the tools are there (also right after installing them).
  $effect(() => {
    if (!maybeVideo || !toolsReady || video || resolving || videoError) return;
    const u = url.trim();
    const mine = token;
    resolving = true;
    api
      .videoInfo(u)
      .then((v) => {
        if (mine === token) video = v;
      })
      .catch((e) => {
        if (mine === token) videoError = String(e);
      })
      .finally(() => {
        if (mine === token) resolving = false;
      });
  });

  // Read a torrent's file list (a magnet asks peers first).
  $effect(() => {
    if (!torrentish || tpreview || tloading || terror) return;
    const u = url.trim();
    const mine = token;
    tloading = true;
    api
      .torrentPreview(u)
      .then((p) => {
        if (mine !== token) return;
        tpreview = p;
        tchosen = new Set(p.files.map((f) => f.index));
      })
      .catch((e) => {
        if (mine === token) terror = String(e);
      })
      .finally(() => {
        if (mine === token) tloading = false;
      });
  });

  async function openTorrentFile() {
    const path = await run(api.pickTorrentFile());
    if (path) {
      url = path;
      fromClip = false;
    }
  }

  async function submitTorrent(start: boolean) {
    if (!tpreview || tchosen.size === 0) return;
    const added = await run(
      api.addTorrent({
        infoHash: tpreview.infoHash,
        onlyFiles: tchosen.size < tpreview.files.length ? [...tchosen] : null,
        extraTrackers: ttrackers.split(/\s+/).filter(Boolean),
        saveDir: folder.trim() || null,
        start,
      }),
    );
    if (added) {
      close();
      select(added.id);
    }
  }

  async function getTools() {
    installing = true;
    await run(api.installTools());
    installing = false;
  }

  function close() {
    app.adding = false;
  }

  async function submit(start: boolean, allowPage = false) {
    if (video) return submitVideo(start);
    if (torrentish) return submitTorrent(start);
    if (!valid || ((isPage || videoHost(url.trim())) && !allowPage)) return;
    const added = await run(
      api.add({
        url: url.trim(),
        fileName: name.trim() || null,
        saveDir: folder.trim() || null,
        connections: lines,
        start,
        allowPage,
      }),
    );
    if (added) {
      close();
      select(added.id);
    }
  }

  async function submitVideo(start: boolean) {
    if (!video || !chosen) return;
    const added = await run(
      api.addVideo({
        url: video.url || url.trim(),
        option: chosen,
        title: video.title,
        channel: video.channel,
        duration: video.duration,
        thumbnail: video.thumbnail,
        site: video.site,
        saveDir: folder.trim() || null,
        start,
        playlist,
      }),
    );
    if (added?.length) {
      close();
      select(added[0].id);
    }
  }

  let sheet = $state<HTMLDivElement>();

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') close();
    if (e.key === 'Tab' && sheet) {
      // Keep focus inside the sheet.
      const f = [...sheet.querySelectorAll<HTMLElement>('input:not([type=radio]), input[type=radio]:checked, select, button:not([disabled])')];
      const i = f.indexOf(document.activeElement as HTMLElement);
      if (e.shiftKey && i <= 0) {
        e.preventDefault();
        f[f.length - 1]?.focus();
      } else if (!e.shiftKey && i === f.length - 1) {
        e.preventDefault();
        f[0]?.focus();
      }
    }
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      submit(true);
    }
  }

  const step = $derived(app.tools?.installing);
  const pagePlain = $derived(isPage && !video && !resolving && (!!videoError || !maybeVideo));
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="scrim" onpointerdown={close}></div>
<div class="sheet" bind:this={sheet} role="dialog" aria-modal="true" aria-label="Add a download" tabindex="-1" onkeydown={onKey}>
  <label class="cap" for="add-url">Link to download</label>
  <div class="urlrow">
    <Icon name="link" />
    <input id="add-url" bind:this={input} class="field" bind:value={url} oninput={() => (fromClip = false)} placeholder="https://…" spellcheck="false" autocomplete="off" />
    {#if fromClip}<span class="clip">From clipboard</span>{/if}
  </div>

  <div class="probe" aria-live="polite">
    {#if tloading}
      <span class="cap">{magnet ? 'Asking peers for the file list… (this can take a minute)' : 'Reading the torrent…'}</span>
    {:else if terror}
      <span class="bad">{terror}</span>
    {:else if tpreview}
      <!-- the torrent picker below says it all -->
    {:else if resolving}
      <span class="cap">Reading the video page…</span>
    {:else if video}
      <!-- the picker below says it all -->
    {:else if maybeVideo && !toolsReady}
      <div class="tools">
        <p><b>Is this a video or a song?</b> DasIDM can save it with yt-dlp and ffmpeg: free tools it downloads once from their official releases (about {bytes(app.tools?.installSize ?? 158_000_000)}) and checks before use.</p>
        {#if step}
          <div class="inst">
            <span class="cap">Getting {step.step} · {step.index + 1} of {step.count}</span>
            <span class="track"><span class="fill" style:width="{Math.min(100, (100 * step.downloaded) / (step.size || 1))}%"></span></span>
            <span class="cap">{bytes(step.downloaded)}</span>
          </div>
        {:else if app.tools?.error}
          <span class="bad">{app.tools.error}</span>
        {/if}
      </div>
    {:else if probing}
      <span class="cap">Asking the server…</span>
    {:else if error}
      <span class="bad">{error}. You can still add it; DasIDM will retry.</span>
    {:else if pagePlain}
      <span class="page-line"
        ><Icon name="alert" />
        {videoError && !/isn't supported/.test(videoError) ? `${videoError}.` : 'This link is a web page, not a file.'} DasIDM won't save it unless you ask.</span
      >
    {:else if info}
      <span class="ok-line">
        <b>{bytes(info.size)}</b>
        <span class="tag" class:no={!info.resumable}>{info.resumable ? 'Resumable · multi-line' : 'No resume · single line'}</span>
        {#if info.content_type}<span class="cap">{info.content_type.split(';')[0]}</span>{/if}
      </span>
    {:else if url && !valid}
      <span class="cap">Links start with http:// or https://</span>
    {/if}
  </div>

  {#if video}
    <VideoPicker info={video} bind:chosen bind:playlist />
  {/if}
  {#if tpreview}
    <TorrentPicker preview={tpreview} bind:chosen={tchosen} bind:trackers={ttrackers} />
  {/if}

  <div class="grid">
    {#if !video && !videoHost(url.trim()) && !torrentish}
      <label>
        <span class="cap">Save as</span>
        <input class="field" bind:value={name} placeholder={info?.file_name ?? 'Name from the server'} spellcheck="false" />
      </label>
      <label>
        <span class="cap">Lines</span>
        <select class="field" bind:value={lines}>
          {#each [1, 2, 4, 8, 12, 16, 24, 32] as n}
            <option value={n}>{n}</option>
          {/each}
        </select>
      </label>
    {/if}
    <div class="wide">
      <label class="cap" for="add-folder">Folder</label>
      <div class="folderrow">
        <input id="add-folder" class="field" bind:value={folder} placeholder={app.settings?.categoryFolders ? `${app.settings?.downloadDir}\\<type>` : app.settings?.downloadDir} spellcheck="false" />
        <button class="btn" onclick={browse}><Icon name="folder" />Browse…</button>
      </div>
    </div>
  </div>

  <div class="foot">
    <button class="btn ghost left" onclick={openTorrentFile} title="Open a .torrent file from this PC"><Icon name="type-torrents" />Open .torrent…</button>
    <button class="btn ghost" onclick={close}>Cancel<kbd>Esc</kbd></button>
    {#if torrentish}
      <button class="btn" disabled={!tpreview || tchosen.size === 0} onclick={() => submitTorrent(false)}>Add paused</button>
      <button class="btn primary" disabled={!tpreview || tchosen.size === 0} onclick={() => submitTorrent(true)}><Icon name="down" />Download<kbd>Enter</kbd></button>
    {:else if video}
      <button class="btn" disabled={!chosen} onclick={() => submitVideo(false)}>Add paused</button>
      <button class="btn primary" disabled={!chosen} onclick={() => submitVideo(true)}><Icon name="down" />Download{playlist && video.playlist ? ` ${video.playlist.count}` : ''}<kbd>Enter</kbd></button>
    {:else if maybeVideo && !toolsReady}
      {#if isPage}<button class="btn" onclick={() => submit(true, true)}>Save page anyway</button>{/if}
      <button class="btn primary" disabled={installing || !!step} onclick={getTools}><Icon name="down" />{installing || step ? 'Getting video tools…' : 'Get video tools'}</button>
    {:else if isPage || (videoHost(url.trim()) && (videoError || resolving))}
      <button class="btn" disabled={resolving} onclick={() => submit(true, true)}>Save page anyway</button>
    {:else}
      <button class="btn" disabled={!valid} onclick={() => submit(false)}>Add paused</button>
      <button class="btn primary" disabled={!valid} onclick={() => submit(true)}><Icon name="down" />Download<kbd>Enter</kbd></button>
    {/if}
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 44px 0 0 0;
    background: rgb(0 0 0 / 0.35);
    z-index: 20;
  }
  .sheet {
    position: fixed;
    top: 46px;
    right: 14px;
    width: 560px;
    max-height: calc(100vh - 60px);
    overflow: auto;
    z-index: 21;
    background: #14171c;
    border: 1px solid var(--line-strong);
    border-radius: 12px;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    box-shadow: 0 18px 48px rgb(0 0 0 / 0.5);
    animation: drop 220ms var(--ease);
  }
  @keyframes drop {
    from {
      opacity: 0;
      transform: translateY(-6px);
    }
  }
  .urlrow {
    position: relative;
    color: var(--fog);
  }
  .urlrow :global(svg) {
    position: absolute;
    left: 10px;
    top: 10px;
  }
  .urlrow input {
    height: 36px;
    padding-left: 32px;
    font-size: 14px;
  }
  .clip {
    position: absolute;
    right: 8px;
    top: 9px;
    font-size: 11px;
    padding: 2px 7px;
    border-radius: 9px;
    color: var(--fog);
    border: 1px solid var(--line-strong);
    pointer-events: none;
  }
  .urlrow input:has(+ .clip) {
    padding-right: 112px;
  }
  .folderrow {
    display: flex;
    gap: 6px;
  }
  .folderrow input {
    flex: 1;
    min-width: 0;
  }
  .wide {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .probe {
    min-height: 22px;
    display: flex;
    align-items: center;
  }
  .probe:empty {
    display: none;
  }
  .tools {
    display: flex;
    flex-direction: column;
    gap: 8px;
    line-height: 1.45;
    color: var(--fog);
  }
  .tools b {
    color: var(--ivory);
  }
  .inst {
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: 10px;
    align-items: center;
  }
  .track {
    height: 4px;
    border-radius: 2px;
    background: rgb(255 255 255 / 0.08);
    overflow: hidden;
  }
  .fill {
    display: block;
    height: 100%;
    background: var(--ivory);
    transition: width 240ms linear;
  }
  .ok-line {
    display: flex;
    gap: 10px;
    align-items: center;
  }
  .ok-line b {
    font-family: var(--mono);
    font-stretch: 75%;
    font-size: 15px;
  }
  .tag {
    font-size: 12px;
    padding: 2px 8px;
    border-radius: 10px;
    background: rgb(76 195 138 / 0.15);
    color: var(--ok);
  }
  .tag.no {
    background: rgb(255 212 0 / 0.12);
    color: var(--alert);
  }
  .bad {
    color: var(--bad);
  }
  .page-line {
    display: flex;
    gap: 8px;
    align-items: center;
    color: var(--ivory);
  }
  .page-line :global(svg) {
    color: var(--fog);
    flex: none;
  }
  .grid {
    display: grid;
    grid-template-columns: 1fr 96px;
    gap: 10px;
  }
  .grid label {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .wide {
    grid-column: 1 / -1;
  }
  .foot {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
    margin-top: 4px;
  }
  .foot .left {
    margin-right: auto;
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
</style>
