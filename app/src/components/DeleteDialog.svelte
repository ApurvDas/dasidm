<script lang="ts">
  // The one question before anything leaves the PC: remove from the list, or delete the files too.
  import { api } from '../lib/api';
  import { bytes } from '../lib/format';
  import Icon from '../lib/Icon.svelte';
  import { app, run, toast } from '../lib/state.svelte';

  const ask = $derived(app.ask);
  const items = $derived((ask?.ids ?? []).map((id) => app.items[id]).filter(Boolean));
  const one = $derived(items.length === 1 ? items[0] : null);
  const allDone = $derived(items.every((i) => i.status === 'done'));
  const size = $derived(items.reduce((a, i) => a + (i.status === 'done' ? (i.size ?? 0) : i.downloaded), 0));
  /** Where the files are: the file itself, or a torrent's own folder. */
  const where = $derived.by(() => {
    if (!one) return '';
    if (one.kind === 'torrent' && (one.torrent?.files ?? 1) > 1) return `${one.saveDir}\\${one.fileName}\\`;
    return one.filePath ?? `${one.saveDir}\\${one.fileName}`;
  });
  const what = $derived(
    one?.kind === 'torrent' && (one.torrent?.files ?? 1) > 1 ? `the folder and its ${one.torrent?.files} files` : items.length > 1 ? `${items.length} files` : 'the file',
  );
  let keep = $state<HTMLButtonElement>();
  $effect(() => {
    if (ask) keep?.focus();
  });

  function close() {
    app.ask = null;
  }
  async function remove(files: boolean) {
    if (!ask) return;
    const ids = ask.ids;
    close();
    await run(api.remove(ids, files));
    if (app.selected && ids.includes(app.selected)) app.selected = null;
    toast(files ? (allDone ? `Deleted ${what} from your PC` : 'Removed; the partial data is deleted') : 'Removed from the list; the files stay on your PC');
  }
  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      close();
    }
  }
</script>

{#if ask && items.length}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="scrim" onpointerdown={close}></div>
  <div class="dlg" role="alertdialog" aria-modal="true" aria-labelledby="del-t" aria-describedby="del-d" tabindex="-1" onkeydown={onKey}>
    <h2 id="del-t">
      <Icon name="trash" size={18} />
      {#if !allDone}Remove {items.length === 1 ? 'this download' : `${items.length} downloads`}?{:else if ask.files}Delete from your PC?{:else}Remove from the list?{/if}
    </h2>
    <div id="del-d" class="body">
      {#if one}
        <b class="name">{one.fileName}</b>
        <span class="path" title={where}>{where}</span>
      {/if}
      {#if !allDone}
        <p>It stops, and the {bytes(size)} downloaded so far is deleted.</p>
      {:else if ask.files}
        <p>
          DasIDM deletes {what} ({bytes(size)}) for good{one?.torrent?.seeding ? ' and stops seeding it' : ''}. {items.length > 1 || what !== 'the file' ? 'They don’t' : 'It doesn’t'} go to the Recycle Bin.
        </p>
      {:else}
        <p>The download leaves the list; {what} stays on your PC.</p>
      {/if}
    </div>
    <div class="btns">
      {#if !allDone}
        <button class="btn danger" onclick={() => remove(true)}><Icon name="trash" />Remove</button>
      {:else}
        <button class="btn danger" class:solid={ask.files} onclick={() => remove(true)}><Icon name="trash" />Delete from PC too</button>
        <button class="btn" onclick={() => remove(false)}>Remove from list only</button>
      {/if}
      <button class="btn ghost" bind:this={keep} onclick={close}>Keep<kbd>Esc</kbd></button>
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 50;
    background: rgb(0 0 0 / 0.45);
  }
  .dlg {
    position: fixed;
    z-index: 51;
    left: 50%;
    top: 30%;
    transform: translateX(-50%);
    width: min(480px, calc(100vw - 32px));
    padding: 18px 18px 14px;
    border-radius: 12px;
    background: #1a1f25;
    border: 1px solid var(--line-strong);
    box-shadow: 0 24px 60px rgb(0 0 0 / 0.55);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 9px;
    font-size: 16px;
    font-weight: 600;
  }
  h2 :global(svg) {
    color: var(--bad);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .name {
    font-family: var(--mono);
    font-stretch: 75%;
    font-size: 13px;
    overflow-wrap: anywhere;
  }
  .path {
    font-family: var(--mono);
    font-stretch: 75%;
    font-size: 11.5px;
    color: var(--fog);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  p {
    margin-top: 6px;
    color: var(--fog);
  }
  .btns {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    justify-content: flex-end;
  }
  .solid {
    background: var(--bad);
    border-color: var(--bad);
    color: #2a0b0b;
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
