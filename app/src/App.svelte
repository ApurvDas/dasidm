<script lang="ts">
  import { onMount } from 'svelte';
  import TitleBar from './components/TitleBar.svelte';
  import Board from './components/Board.svelte';
  import Hall from './components/Hall.svelte';
  import Pass from './components/Pass.svelte';
  import SettingsPanel from './components/SettingsPanel.svelte';
  import AddSheet from './components/AddSheet.svelte';
  import StatusBar from './components/StatusBar.svelte';
  import PartsView from './components/PartsView.svelte';
  import { api, inTauri } from './lib/api';
  import { app, init, run, select } from './lib/state.svelte';

  let hall = $state<ReturnType<typeof Hall>>();
  let ready = $state(false);

  onMount(() => {
    if (!inTauri) document.body.classList.add('preview');
    // A video link opened with DasIDM (command line, second launch) goes to the picker.
    const openAdd = (url: string) => {
      api.takePendingAdd().catch(() => null);
      app.adding = url;
    };
    api.onOpenAdd(openAdd);
    init().then(async () => {
      ready = true;
      if (inTauri) {
        const pending = await api.takePendingAdd().catch(() => null);
        if (pending) app.adding = pending;
      }
      // Design preview only (synthetic data): ?select=<id>&parts opens a state for review captures.
      if (!inTauri) {
        const q = new URLSearchParams(location.search);
        const id = q.get('select');
        if (id && app.items[id]) select(id);
        if (q.has('parts')) app.partsOpen = true;
        if (q.has('add')) app.adding = q.get('add') || true;
      }
    });
    const open = (e: Event) => run(api.openFile((e as CustomEvent<string>).detail));
    window.addEventListener('das:open', open);
    return () => window.removeEventListener('das:open', open);
  });

  const typing = (t: EventTarget | null) => t instanceof HTMLElement && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName));

  function onPaste(e: ClipboardEvent) {
    if (typing(e.target) || app.adding) return;
    const text = e.clipboardData?.getData('text')?.trim() ?? '';
    if (/^https?:\/\/\S+$/i.test(text)) {
      e.preventDefault();
      app.adding = text;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (app.adding || typing(e.target)) return;
    const id = app.selected;
    const item = id ? app.items[id] : null;
    if (e.ctrlKey && e.key.toLowerCase() === 'f') {
      e.preventDefault();
      hall?.focusSearch();
    } else if (e.ctrlKey && e.key.toLowerCase() === 'n') {
      e.preventDefault();
      app.adding = true;
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      hall?.move(e.key === 'ArrowDown' ? 1 : -1);
    } else if (e.key === 'Escape') {
      app.confirmDelete = false;
      app.partsOpen = false;
      app.panel = 'pass';
    } else if (e.key.toLowerCase() === 'p' && !e.ctrlKey && item) {
      app.partsOpen = !app.partsOpen;
    } else if (!item) {
      return;
    } else if (e.key === ' ') {
      e.preventDefault();
      if (item.status === 'downloading' || item.status === 'queued') run(api.pause([item.id]));
      else if (item.status === 'paused' || item.status === 'failed') run(api.resume([item.id]));
    } else if (e.key === 'Enter' && item.status === 'done') {
      run(api.openFile(item.id));
    } else if (e.key === 'Delete') {
      app.panel = 'pass';
      app.confirmDelete = true;
    }
  }
</script>

<svelte:window onkeydown={onKey} onpaste={onPaste} />

<div class="shell" class:ready>
  <TitleBar />
  <main>
    <div class="left" class:covered={app.partsOpen && app.selected}>
      <Board />
      <Hall bind:this={hall} />
      {#if app.partsOpen && app.selected}
        <PartsView />
      {/if}
    </div>
    {#if app.panel === 'settings' && app.settings}
      <SettingsPanel />
    {:else}
      <Pass />
    {/if}
  </main>
  <StatusBar />
</div>

{#if app.adding}
  <AddSheet />
{/if}

<style>
  .shell {
    height: 100%;
    display: grid;
    /* minmax(0, 1fr): no child (e.g. the title bar) may push the window wider than itself. */
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: auto minmax(0, 1fr) auto;
    opacity: 0;
    transition: opacity 180ms var(--ease);
  }
  .shell.ready {
    opacity: 1;
  }
  main {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 360px;
    gap: 10px;
    padding: 0 10px;
    min-height: 0;
  }
  /* The parts view takes the board's and list's place; they stay mounted but unseen. */
  .left.covered > :global(:not(.parts)) {
    visibility: hidden;
  }
  .left {
    position: relative;
    min-width: 0;
    display: grid;
    grid-template-rows: auto minmax(170px, 1fr);
    gap: 10px;
    min-height: 0;
  }
  @media (max-width: 1100px) {
    main {
      grid-template-columns: minmax(0, 1fr) 300px;
    }
  }
</style>
