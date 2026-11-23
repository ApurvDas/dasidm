<script lang="ts">
  // The Add sheet for a torrent: what is in it, which files to fetch, and extra trackers.
  import { bytes } from '../lib/format';
  import Icon from '../lib/Icon.svelte';
  import { app } from '../lib/state.svelte';
  import type { TorrentPreview } from '../lib/types';
  import FileTree from './FileTree.svelte';

  let {
    preview,
    chosen = $bindable(),
    trackers = $bindable(''),
  }: { preview: TorrentPreview; chosen: Set<number>; trackers: string } = $props();

  const total = $derived(preview.files.filter((f) => chosen.has(f.index)).reduce((a, f) => a + f.size, 0));
  const all = $derived(chosen.size === preview.files.length);
  const defaults = $derived((app.settings?.defaultTrackers ?? '').split(/\s+/).filter(Boolean).length);

  /** Folders inside the torrent (its own top folder included). */
  const folders = $derived(
    new Set(preview.files.flatMap((f) => f.path.split('\\').slice(0, -1).map((_, i, a) => a.slice(0, i + 1).join('\\')))).size,
  );
  function setAll(on: boolean) {
    chosen = new Set(on ? preview.files.map((f) => f.index) : []);
  }
</script>

<div class="picker">
  <div class="head">
    <Icon name="type-torrents" size={18} />
    <div class="title">
      <b title={preview.name}>{preview.name}</b>
      <span class="meta"
        >{preview.files.length}
        {preview.files.length === 1 ? 'file' : 'files'}{folders ? ` in ${folders} ${folders === 1 ? 'folder' : 'folders'}` : ''} · {bytes(preview.size)} · {preview.pieces.toLocaleString()} pieces of {bytes(preview.pieceSize)}{#if preview.private}
          · <span class="priv">Private: trackers only, no DHT</span>{/if}</span
      >
    </div>
  </div>

  {#if preview.files.length > 1}
    <div class="bar">
      <label class="all">
        <input type="checkbox" checked={all} indeterminate={!all && chosen.size > 0} onchange={() => setAll(!all)} />
        <span>All files</span>
      </label>
      <span class="sum">Fetching {chosen.size} of {preview.files.length} · <b>{bytes(total)}</b></span>
    </div>
    <div class="list">
      <FileTree files={preview.files} root={preview.name} bind:chosen editable showRoot />
    </div>
  {/if}

  <details class="trk">
    <summary>
      Trackers <span class="n">{preview.trackers.length} in the torrent{defaults ? ` + ${defaults} from Settings` : ''}</span>
    </summary>
    <textarea class="field" bind:value={trackers} rows="3" spellcheck="false" placeholder="Extra trackers, one per line (udp://…, https://…)"></textarea>
    <span class="hint">You can add, edit or remove trackers later in the torrent view too.</span>
  </details>
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
    overflow-wrap: anywhere;
  }
  .meta {
    font-size: 12px;
  }
  .priv {
    color: var(--ivory);
  }
  .bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
  }
  .all {
    display: flex;
    gap: 8px;
    align-items: center;
    font-weight: 600;
  }
  .sum {
    color: var(--fog);
    font-size: 12.5px;
  }
  .sum b {
    color: var(--ivory);
    font-family: var(--mono);
    font-stretch: 75%;
  }
  input[type='checkbox'] {
    accent-color: var(--ivory);
    width: 14px;
    height: 14px;
    flex: none;
  }
  .list {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--line);
    border-radius: 8px;
    overflow: auto;
    max-height: min(260px, 36vh);
  }
  .trk summary {
    cursor: pointer;
    font-weight: 600;
  }
  .trk .n {
    font-weight: 400;
    color: var(--fog);
    margin-left: 6px;
  }
  .trk textarea {
    margin-top: 8px;
    width: 100%;
    resize: vertical;
    font-family: var(--mono);
    font-stretch: 75%;
    font-size: 12px;
    height: auto;
    padding: 8px 10px;
  }
  .hint {
    display: block;
    margin-top: 4px;
    font-size: 12px;
    color: var(--fog);
  }
</style>
