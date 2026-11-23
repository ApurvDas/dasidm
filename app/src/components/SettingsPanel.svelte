<script lang="ts">
  import { api } from '../lib/api';
  import { app, run, toast } from '../lib/state.svelte';
  import Icon from '../lib/Icon.svelte';
  import type { BrowserStatus, Settings } from '../lib/types';
  import { when } from '../lib/format';

  let s = $state<Settings>({ ...(app.settings as Settings) });
  let limitKb = $state(Math.round((app.settings?.speedLimit ?? 0) / 1024));
  let upKb = $state(Math.round((app.settings?.uploadLimit ?? 0) / 1024));
  const restartNeeded = $derived(
    !!app.settings &&
      (s.torrentPort !== app.settings.torrentPort || s.torrentUpnp !== app.settings.torrentUpnp || s.torrentDht !== app.settings.torrentDht),
  );

  async function registerHandler() {
    const ok = await run(api.registerTorrentHandler().then(() => true));
    if (ok) toast('Magnet links and .torrent files now open in DasIDM');
  }

  // Firefox: is the helper registered, and has the extension been in touch?
  let browser = $state<BrowserStatus | null>(null);
  $effect(() => {
    api.browserStatus().then((b) => (browser = b)).catch(() => {});
  });
  async function repair() {
    const b = await run(api.repairBrowser());
    if (b) {
      browser = b;
      toast('Firefox can reach DasIDM again');
    }
  }

  async function browse() {
    const picked = await run(api.pickFolder(s.downloadDir));
    if (picked) s.downloadDir = picked;
  }

  async function save() {
    const next = {
      ...s,
      speedLimit: Math.max(0, Math.round(limitKb)) * 1024,
      uploadLimit: Math.max(0, Math.round(upKb)) * 1024,
      torrentPort: Math.min(65535, Math.max(1024, Math.round(s.torrentPort))),
      seedRatio: Math.max(0, Number(s.seedRatio) || 0),
      seedMinutes: Math.max(0, Math.round(s.seedMinutes)),
    };
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

  <div class="field-group">
    <label class="cap" for="set-dir">Download folder</label>
    <div class="folderrow">
      <input id="set-dir" class="field" bind:value={s.downloadDir} spellcheck="false" />
      <button class="btn" onclick={browse}><Icon name="folder" />Browse…</button>
    </div>
  </div>
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

  <h4>Torrents</h4>
  <div class="two">
    <label>
      <span class="cap">Seed until ratio</span>
      <input class="field" type="number" min="0" step="0.1" bind:value={s.seedRatio} />
    </label>
    <label>
      <span class="cap">Or minutes</span>
      <input class="field" type="number" min="0" step="10" bind:value={s.seedMinutes} />
    </label>
  </div>
  <span class="hint under">Finished torrents upload to others until either limit is reached. 0 means no limit.</span>
  <label>
    <span class="cap">Upload limit, KB/s</span>
    <input class="field" type="number" min="0" step="64" bind:value={upKb} />
  </label>
  <label>
    <span class="cap">Trackers for every new torrent</span>
    <textarea class="field area" rows="3" spellcheck="false" bind:value={s.defaultTrackers} placeholder="One per line: udp://…, https://…"></textarea>
  </label>
  <div class="two">
    <label>
      <span class="cap">Incoming port</span>
      <input class="field" type="number" min="1024" max="65535" bind:value={s.torrentPort} />
    </label>
    <div class="checks">
      <label class="check"><input type="checkbox" bind:checked={s.torrentUpnp} /><span>Open the port on the router <em>(UPnP)</em></span></label>
      <label class="check"><input type="checkbox" bind:checked={s.torrentDht} /><span>Find peers without trackers <em>(DHT)</em></span></label>
    </div>
  </div>
  {#if restartNeeded}<span class="hint under">Port, UPnP and DHT changes apply the next time DasIDM starts.</span>{/if}
  <button class="btn" onclick={registerHandler}><Icon name="type-torrents" />Use DasIDM for magnets and .torrent files</button>

  <h4>Firefox</h4>
  <div class="browser">
    <span class="st" class:ok={browser?.registered && browser?.lastSeen} class:bad={browser && !browser.registered}>
      {#if !browser}Checking…
      {:else if !browser.registered}Not set up{browser.error ? `: ${browser.error}` : ''}
      {:else if browser.lastSeen}Extension connected · {when(browser.lastSeen)}
      {:else}Ready; the extension hasn't connected yet{/if}
    </span>
    <span class="hint">
      With the <b>DasIDM – Download Manager</b> add-on, files you download in Firefox open here instead. Get it from Firefox Add-ons; the
      extension folder is for trying a development copy (<b>about:debugging</b> → Load Temporary Add-on…).
    </span>
    <div class="btns">
      <button class="btn primary" onclick={() => run(api.openAddonPage())}><Icon name="open" />Get the Firefox add-on</button>
      <button class="btn" onclick={() => run(api.openExtensionFolder())}><Icon name="folder" />Extension folder</button>
      <button class="btn ghost" onclick={repair}><Icon name="restart" />Repair</button>
    </div>
  </div>

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
  .field-group {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .browser {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .browser .st {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 600;
  }
  .browser .st::before {
    content: '';
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--steel);
  }
  .browser .st.ok::before {
    background: var(--ok);
  }
  .browser .st.bad {
    color: var(--bad);
  }
  .browser .st.bad::before {
    background: var(--bad);
  }
  .btns {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .folderrow {
    display: flex;
    gap: 6px;
  }
  .folderrow input {
    flex: 1;
    min-width: 0;
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
  h4 {
    margin-top: 6px;
    padding-top: 12px;
    border-top: 1px solid var(--line);
    font-family: var(--mono);
    font-stretch: 75%;
    font-size: 14px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
  .under {
    margin-top: -8px;
  }
  .checks {
    display: flex;
    flex-direction: column;
    gap: 8px;
    justify-content: flex-end;
  }
  .area {
    height: auto;
    resize: vertical;
    padding: 8px 10px;
    font-family: var(--mono);
    font-stretch: 75%;
    font-size: 12px;
  }
  .foot {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
    margin-top: auto;
  }
</style>
