<script lang="ts">
  import { api, inTauri, windowControls } from '../lib/api';
  import { app, matchesFilter, run, type Filter } from '../lib/state.svelte';
  import Icon from '../lib/Icon.svelte';
  import Mark from '../lib/Mark.svelte';
  import type { Status } from '../lib/types';

  // Each filter tab carries the same state mark as its rows, so a bare count is never unlabelled.
  const markFor: Record<Exclude<Filter, 'all'>, Status> = {
    active: 'downloading',
    queued: 'queued',
    paused: 'paused',
    done: 'done',
    failed: 'failed',
  };

  const tabs: [Filter, string][] = [
    ['all', 'All'],
    ['active', 'Loading'],
    ['queued', 'Waiting'],
    ['paused', 'Paused'],
    ['done', 'Finished'],
    ['failed', 'Failed'],
  ];
  const counts = $derived.by(() => {
    const c: Record<string, number> = {};
    const list = Object.values(app.items);
    for (const [f] of tabs) c[f] = list.filter((i) => matchesFilter(i.status, f)).length;
    return c;
  });
  // Design preview can draw the window controls too (?chrome) so captures match the app.
  const showChrome = typeof location !== 'undefined' && location.search.includes('chrome');
  const anyActive = $derived(Object.values(app.items).some((i) => i.status === 'downloading' || i.status === 'queued'));

  async function win(action: 'minimize' | 'toggleMaximize' | 'close') {
    const w = await windowControls();
    await w?.[action]();
  }
</script>

<header class="title" data-tauri-drag-region>
  <span class="brand" data-tauri-drag-region>
    <svg width="18" height="18" viewBox="0 0 16 16" aria-hidden="true"><rect x="1" y="1" width="14" height="14" rx="3" fill="#0b0d10" stroke="#2a3038" /><path d="M8 4v5M5.5 7.5L8 10l2.5-2.5" stroke="#ffd400" stroke-width="1.6" fill="none" stroke-linecap="round" stroke-linejoin="round" /><rect x="4.5" y="11.5" width="7" height="1.4" rx=".7" fill="#ffd400" /></svg>
    DASIDM
  </span>

  <nav class="tabs" aria-label="Filter downloads">
    {#each tabs as [f, label]}
      {#if f === 'all' || counts[f] > 0 || app.filter === f}
        <button
          class="tab"
          class:on={app.filter === f}
          class:keep={f === 'all' || f === 'failed'}
          aria-pressed={app.filter === f}
          aria-label="{label} ({counts[f]})"
          title={label}
          onclick={() => (app.filter = f)}
        >
          {#if f !== 'all'}<Mark status={markFor[f]} />{/if}<span class="tl">{label}</span><span class="n" class:bad={f === 'failed'}>{counts[f]}</span>
        </button>
      {/if}
    {/each}
  </nav>

  <div class="spacer" data-tauri-drag-region></div>

  <div class="acts">
    {#if anyActive}
      <button class="btn ghost" onclick={() => run(api.pauseAll())} aria-label="Pause all"><Icon name="pause" /><span class="label">Pause all</span></button>
    {:else if counts.paused > 0}
      <button class="btn ghost" onclick={() => run(api.resumeAll())} aria-label="Resume all"><Icon name="play" /><span class="label">Resume all</span></button>
    {/if}
    <button class="btn ghost" class:on={app.panel === 'settings'} onclick={() => (app.panel = app.panel === 'settings' ? 'pass' : 'settings')} aria-pressed={app.panel === 'settings'}><Icon name="gear" />Settings</button>
    <button class="btn primary" onclick={() => (app.adding = true)}><Icon name="plus" />Add URL</button>
  </div>

  {#if inTauri || showChrome}
    <div class="win">
      <button onclick={() => win('minimize')} aria-label="Minimize"><Icon name="min" /></button>
      <button onclick={() => win('toggleMaximize')} aria-label="Maximize"><Icon name="max" size={14} /></button>
      <button class="x" onclick={() => win('close')} aria-label="Close to tray"><Icon name="close" /></button>
    </div>
  {/if}
</header>

<style>
  .title {
    min-width: 0;
    height: 44px;
    display: flex;
    align-items: center;
    gap: 14px;
    padding-left: 14px;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 9px;
    font-family: var(--mono);
    font-stretch: 75%;
    font-weight: 700;
    font-size: 15px;
    letter-spacing: 0.08em;
    padding-right: 6px;
  }
  .tabs {
    display: flex;
    gap: 2px;
    min-width: 0;
    overflow: hidden;
    flex: 0 1 auto;
  }
  .brand,
  .acts,
  .win {
    flex: none;
  }
  /* The failure signal never clips: All and Failed keep their labels; others give way first. */
  .tab.keep {
    flex: none;
  }
  @media (max-width: 1000px) {
    .tab:not(.keep):not(.on) .tl {
      display: none;
    }
  }
  @media (max-width: 1100px) {
    .tab {
      padding: 0 7px;
      gap: 5px;
    }
    .acts .label {
      display: none;
    }
  }
  .tab {
    height: 30px;
    padding: 0 10px;
    border-radius: 6px;
    display: flex;
    align-items: center;
    gap: 7px;
    color: var(--fog);
    font-weight: 600;
    transition: background 120ms var(--ease), color 120ms var(--ease);
  }
  .tab:hover {
    color: var(--ivory);
    background: var(--hover);
  }
  .tab.on {
    color: var(--ivory);
    background: rgb(255 255 255 / 0.09);
  }
  .n {
    font-family: var(--mono);
    font-stretch: 75%;
    font-size: 11px;
    color: var(--fog);
  }
  .n.bad {
    color: var(--bad);
  }
  .spacer {
    flex: 1;
    align-self: stretch;
  }
  .acts {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .acts .on {
    background: rgb(255 255 255 / 0.09);
  }
  .win {
    display: flex;
    align-self: stretch;
    margin-left: 6px;
  }
  .win button {
    width: 46px;
    display: grid;
    place-items: center;
    color: var(--fog);
  }
  .win button:hover {
    background: rgb(255 255 255 / 0.08);
    color: var(--ivory);
  }
  .win .x:hover {
    background: #c42b1c;
    color: #fff;
  }
</style>
