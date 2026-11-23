<script lang="ts">
  import { app } from '../lib/state.svelte';
  import { speed } from '../lib/format';
</script>

<footer class="status">
  <button onclick={() => (app.panel = 'settings')}>
    Speed limit <b>{app.settings?.speedLimit ? speed(app.settings.speedLimit) : 'None'}</b>
  </button>
  <button onclick={() => (app.panel = 'settings')}>Lines <b>{app.settings?.connections ?? '—'}</b></button>
  <button onclick={() => (app.panel = 'settings')}>At once <b>{app.settings?.maxConcurrent ?? '—'}</b></button>
  <span class="dir" title={app.settings?.downloadDir}>Saving to <b>{app.settings?.downloadDir ?? '—'}</b></span>
  {#if app.toast}
    <span class="toast" role="status">{app.toast}</span>
  {/if}
</footer>

<style>
  .status {
    height: 28px;
    display: flex;
    align-items: center;
    gap: 18px;
    padding: 0 14px;
    font-size: 12px;
    color: var(--fog);
  }
  button {
    color: inherit;
    font-size: inherit;
  }
  button:hover {
    color: var(--ivory);
  }
  b {
    color: var(--ivory);
    font-weight: 600;
  }
  .dir {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }
  .toast {
    margin-left: auto;
    color: var(--alert);
    white-space: nowrap;
  }
</style>
