<script lang="ts">
  // Convert music the user owns into another format or quality (ffmpeg, on this PC).
  import { api } from '../lib/api';
  import { app, run, select } from '../lib/state.svelte';
  import { bytes } from '../lib/format';
  import Icon from '../lib/Icon.svelte';
  import { PRESETS, type ConvertPlan, type Preset } from '../lib/types';

  let paths = $state<string[]>([...(app.convert ?? [])]);
  let preset = $state<Preset>('mp3-320');
  let outDir = $state('');
  let plan = $state<ConvertPlan | null>(null);
  let loading = $state(false);
  let installing = $state(false);
  let busy = $state(false);

  $effect(() => {
    const p = [...paths];
    const pr = preset;
    if (!p.length) {
      plan = null;
      return;
    }
    let live = true;
    loading = true;
    api
      .convertPlan(p, pr, null)
      .then((r) => {
        if (live) plan = r;
      })
      .catch(() => {})
      .finally(() => {
        if (live) loading = false;
      });
    return () => (live = false);
  });

  const chosen = $derived(PRESETS.find((p) => p.id === preset)!);
  const files = $derived(plan?.files ?? []);
  const willConvert = $derived(files.filter((f) => f.verdict?.kind !== 'skip'));
  const skipped = $derived(files.filter((f) => f.verdict?.kind === 'skip'));
  const warnings = $derived.by(() => {
    const m = new Map<string, number>();
    for (const f of files) if (f.verdict?.kind === 'warn') m.set(f.verdict.note, (m.get(f.verdict.note) ?? 0) + 1);
    return [...m];
  });
  const inputBytes = $derived(willConvert.reduce((a, f) => a + f.size, 0));
  // A rough size for the result: lossy presets scale with bitrate against lossless sources at
  // about 1000 kbps; 16-bit FLAC from hi-res is about half; other lossless is about the same.
  const estimate = $derived(chosen.kbps ? Math.round((inputBytes * chosen.kbps) / 1000) : chosen.id === 'flac-16' ? Math.round(inputBytes * 0.5) : inputBytes);

  async function addFiles() {
    const picked = await run(api.pickAudioFiles());
    if (picked?.length) paths = [...new Set([...paths, ...picked])];
  }
  async function addFolder() {
    const picked = await run(api.pickFolder(paths[0] ?? app.settings?.downloadDir));
    if (picked) paths = [...new Set([...paths, picked])];
  }
  async function pickOut() {
    const picked = await run(api.pickFolder(outDir || plan?.outDir || undefined));
    if (picked) outDir = picked;
  }
  async function getTools() {
    installing = true;
    await run(api.installTools());
    installing = false;
    paths = [...paths]; // read the files again now that ffprobe is there
  }
  function close() {
    app.convert = null;
  }
  async function convert() {
    if (!willConvert.length || busy) return;
    busy = true;
    const added = await run(api.addConvert(paths, preset, outDir.trim() || null));
    busy = false;
    if (added) {
      close();
      select(added.id);
    }
  }
  const name = (p: string) => p.split(/[\\/]/).filter(Boolean).pop() ?? p;
  const step = $derived(app.tools?.installing);
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && !app.ask && close()} />
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="scrim" onpointerdown={close}></div>
<div class="sheet" role="dialog" aria-modal="true" aria-label="Convert music" tabindex="-1">
  <div class="top">
    <h3><Icon name="type-music" />Convert music</h3>
    <span class="note">Your own files, on this PC. The originals are never changed.</span>
  </div>

  <div class="picks">
    <button class="btn" onclick={addFiles}><Icon name="plus" />Add files…</button>
    <button class="btn" onclick={addFolder}><Icon name="folder" />Add folder…</button>
    {#if paths.length}<button class="btn ghost" onclick={() => (paths = [])}>Clear</button>{/if}
  </div>

  {#if plan && !plan.ffmpegReady}
    <div class="tools">
      <p>Converting uses ffmpeg, a free tool DasIDM downloads once from its official release and checks before use.</p>
      {#if step}<span class="note">Getting {step.step} · {step.index + 1} of {step.count} · {bytes(step.downloaded)}</span>{/if}
      <button class="btn primary" disabled={installing || !!step} onclick={getTools}><Icon name="down" />{installing || step ? 'Getting the tools…' : 'Get media tools'}</button>
    </div>
  {/if}

  {#if paths.length}
    <div class="files" aria-live="polite">
      {#if loading && !plan}
        <span class="note">Reading the files…</span>
      {:else if files.length === 0}
        <span class="note">No audio files in {paths.map(name).join(', ')}.</span>
      {:else}
        <ul>
          {#each files.slice(0, 200) as f (f.path)}
            <li class:skip={f.verdict?.kind === 'skip'}>
              <span class="fn" title={f.path}>{f.name}</span>
              <span class="fmt">{f.format ?? '—'}</span>
              {#if f.verdict?.kind === 'skip'}<span class="tag">Left out · {f.verdict.note}</span>{/if}
            </li>
          {/each}
          {#if files.length > 200}<li class="more">and {files.length - 200} more</li>{/if}
        </ul>
      {/if}
    </div>
  {:else}
    <p class="empty">Add music files or a whole album folder. Tip: drop files on the window, or use Convert… on a finished download.</p>
  {/if}

  <label class="wide">
    <span class="cap">Convert to</span>
    <select class="field" bind:value={preset}>
      <optgroup label="Lossy: smaller files">
        {#each PRESETS.filter((p) => !p.lossless) as p}<option value={p.id}>{p.label}</option>{/each}
      </optgroup>
      <optgroup label="Lossless: every bit kept">
        {#each PRESETS.filter((p) => p.lossless) as p}<option value={p.id}>{p.label}</option>{/each}
      </optgroup>
    </select>
    {#if chosen.hint}<span class="note">{chosen.hint}</span>{/if}
  </label>

  <div class="wide">
    <span class="cap">Save to</span>
    <div class="folderrow">
      <input class="field" bind:value={outDir} placeholder={plan?.outDir ? `Next to the originals: ${plan.outDir}` : 'Next to the originals, in a folder named after the format'} spellcheck="false" />
      <button class="btn" onclick={pickOut}><Icon name="folder" />Browse…</button>
    </div>
  </div>

  {#if files.length}
    <div class="sum">
      {#each warnings as [note, n]}
        <p class="warn"><Icon name="alert" />{note} ({n} {n === 1 ? 'file' : 'files'}).</p>
      {/each}
      {#if skipped.length}<p class="note">{skipped.length} {skipped.length === 1 ? 'file is' : 'files are'} left out: nothing to gain.</p>{/if}
      {#if plan && !plan.keepsCover}<p class="note">Cover art isn't kept in this format; tags are.</p>{/if}
      {#if willConvert.length}<p class="note">{willConvert.length} {willConvert.length === 1 ? 'file' : 'files'}, about {bytes(estimate)} when done.</p>{/if}
    </div>
  {/if}

  <div class="foot">
    <button class="btn ghost" onclick={close}>Cancel<kbd>Esc</kbd></button>
    <button class="btn primary" disabled={!willConvert.length || busy || (plan ? !plan.ffmpegReady : true)} onclick={convert}>
      <Icon name="restart" />Convert{willConvert.length ? ` ${willConvert.length}` : ''}
    </button>
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
    gap: 12px;
    box-shadow: 0 18px 48px rgb(0 0 0 / 0.5);
    animation: drop 220ms var(--ease);
  }
  @keyframes drop {
    from {
      opacity: 0;
      transform: translateY(-6px);
    }
  }
  .top {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  h3 {
    margin: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 15px;
    color: var(--ivory);
  }
  .note {
    color: var(--fog);
    font-size: 12px;
    line-height: 1.4;
    margin: 0;
  }
  .picks {
    display: flex;
    gap: 6px;
  }
  .tools {
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: flex-start;
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: 8px;
  }
  .tools p {
    margin: 0;
    font-size: 12.5px;
    color: var(--fog);
  }
  .files ul {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 190px;
    overflow: auto;
    border: 1px solid var(--line);
    border-radius: 8px;
  }
  .files li {
    display: flex;
    gap: 10px;
    align-items: baseline;
    padding: 6px 10px;
    font-size: 12.5px;
  }
  .files li + li {
    border-top: 1px solid var(--line);
  }
  .files li.skip {
    opacity: 0.6;
  }
  .fn {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ivory);
  }
  .fmt {
    flex: none;
    color: var(--fog);
    font-family: var(--mono);
    font-size: 11.5px;
  }
  .tag {
    flex: none;
    font-size: 11px;
    color: var(--fog);
  }
  .more {
    color: var(--fog);
  }
  .empty {
    margin: 0;
    padding: 14px;
    border: 1px dashed var(--line-strong);
    border-radius: 8px;
    color: var(--fog);
    font-size: 12.5px;
  }
  .wide {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .folderrow {
    display: flex;
    gap: 6px;
  }
  .folderrow input {
    flex: 1;
    min-width: 0;
  }
  .sum {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .warn {
    margin: 0;
    display: flex;
    gap: 6px;
    align-items: flex-start;
    color: var(--alert);
    font-size: 12px;
  }
  .warn :global(svg) {
    flex: none;
    margin-top: 1px;
  }
  .foot {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
  }
</style>
