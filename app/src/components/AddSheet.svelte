<script lang="ts">
  import { api } from '../lib/api';
  import { app, run, select } from '../lib/state.svelte';
  import { bytes } from '../lib/format';
  import Icon from '../lib/Icon.svelte';
  import type { ProbeInfo } from '../lib/types';

  let url = $state(typeof app.adding === 'string' ? app.adding : '');
  let name = $state('');
  let folder = $state('');
  let lines = $state(app.settings?.connections ?? 16);
  let info = $state<ProbeInfo | null>(null);
  let probing = $state(false);
  let error = $state('');
  let input = $state<HTMLInputElement>();
  let token = 0;

  $effect(() => {
    input?.focus();
    input?.select();
  });

  const valid = $derived(/^https?:\/\/\S+$/i.test(url.trim()));

  $effect(() => {
    const u = url.trim();
    info = null;
    error = '';
    if (!/^https?:\/\/\S+$/i.test(u)) return;
    const mine = ++token;
    probing = true;
    const t = setTimeout(async () => {
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

  function close() {
    app.adding = false;
  }

  async function submit(start: boolean) {
    if (!valid) return;
    const added = await run(
      api.add({
        url: url.trim(),
        fileName: name.trim() || null,
        saveDir: folder.trim() || null,
        connections: lines,
        start,
      }),
    );
    if (added) {
      close();
      select(added.id);
    }
  }

  let sheet = $state<HTMLDivElement>();

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') close();
    if (e.key === 'Tab' && sheet) {
      // Keep focus inside the sheet.
      const f = [...sheet.querySelectorAll<HTMLElement>('input, select, button:not([disabled])')];
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
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="scrim" onpointerdown={close}></div>
<div class="sheet" bind:this={sheet} role="dialog" aria-modal="true" aria-label="Add a download" tabindex="-1" onkeydown={onKey}>
  <label class="cap" for="add-url">Link to download</label>
  <div class="urlrow">
    <Icon name="link" />
    <input id="add-url" bind:this={input} class="field" bind:value={url} placeholder="https://…" spellcheck="false" autocomplete="off" />
  </div>

  <div class="probe" aria-live="polite">
    {#if probing}
      <span class="cap">Asking the server…</span>
    {:else if error}
      <span class="bad">{error}. You can still add it; DasIDM will retry.</span>
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

  <div class="grid">
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
    <label class="wide">
      <span class="cap">Folder</span>
      <input class="field" bind:value={folder} placeholder={app.settings?.categoryFolders ? `${app.settings?.downloadDir}\\<type>` : app.settings?.downloadDir} spellcheck="false" />
    </label>
  </div>

  <div class="foot">
    <button class="btn ghost" onclick={close}>Cancel<kbd>Esc</kbd></button>
    <button class="btn" disabled={!valid} onclick={() => submit(false)}>Add paused</button>
    <button class="btn primary" disabled={!valid} onclick={() => submit(true)}><Icon name="down" />Download<kbd>Enter</kbd></button>
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
  .probe {
    min-height: 22px;
    display: flex;
    align-items: center;
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
