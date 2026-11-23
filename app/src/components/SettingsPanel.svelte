<script lang="ts">
  import { api } from '../lib/api';
  import { app, run, toast } from '../lib/state.svelte';
  import Icon from '../lib/Icon.svelte';
  import type { Settings } from '../lib/types';

  let s = $state<Settings>({ ...(app.settings as Settings) });
  let limitKb = $state(Math.round((app.settings?.speedLimit ?? 0) / 1024));

  async function save() {
    const next = { ...s, speedLimit: Math.max(0, Math.round(limitKb)) * 1024 };
    const ok = await run(api.setSettings(next).then(() => true));
    if (ok) {
      app.settings = next;
      app.panel = 'pass';
      toast('Settings saved');
    }
  }
</script>

<aside class="panel" aria-label="Settings">
  <header>
    <h3>Settings</h3>
    <button class="btn ghost" onclick={() => (app.panel = 'pass')} aria-label="Close settings"><Icon name="close" /></button>
  </header>

  <label>
    <span class="cap">Download folder</span>
    <input class="field" bind:value={s.downloadDir} spellcheck="false" />
  </label>
  <label class="check">
    <input type="checkbox" bind:checked={s.categoryFolders} />
    <span>Sort into folders by type <em>(Video, Music, Programs…)</em></span>
  </label>

  <div class="two">
    <label>
      <span class="cap">At the same time</span>
      <select class="field" bind:value={s.maxConcurrent}>
        {#each [1, 2, 3, 4, 5, 6, 8, 10] as n}<option value={n}>{n} downloads</option>{/each}
      </select>
    </label>
    <label>
      <span class="cap">Lines per download</span>
      <select class="field" bind:value={s.connections}>
        {#each [1, 2, 4, 8, 12, 16, 24, 32] as n}<option value={n}>{n}</option>{/each}
      </select>
    </label>
  </div>

  <label>
    <span class="cap">Speed limit, KB/s</span>
    <input class="field" type="number" min="0" step="64" bind:value={limitKb} />
    <span class="hint">0 means no limit. Applies to all downloads together.</span>
  </label>

  <label class="check">
    <input type="checkbox" bind:checked={s.closeToTray} />
    <span>Keep downloading in the tray when the window closes <em>(the window is unloaded to save memory)</em></span>
  </label>

  <div class="foot">
    <button class="btn ghost" onclick={() => (app.panel = 'pass')}>Cancel</button>
    <button class="btn primary" onclick={save}><Icon name="check" />Save</button>
  </div>
</aside>

<style>
  .panel {
    background: var(--glass-pass);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    overflow: auto;
    min-height: 0;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  h3 {
    font-family: var(--mono);
    font-stretch: 75%;
    font-size: 20px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .check {
    flex-direction: row;
    align-items: flex-start;
    gap: 10px;
  }
  .check input {
    margin-top: 3px;
    accent-color: var(--alert);
    width: 15px;
    height: 15px;
  }
  em {
    font-style: normal;
    color: var(--fog);
  }
  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }
  .hint {
    color: var(--fog);
    font-size: 12px;
  }
  .foot {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
    margin-top: auto;
  }
</style>
