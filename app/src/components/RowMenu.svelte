<script lang="ts">
  // The right-click menu of a download (also the ⋯ button and the keyboard's menu key).
  import { tick } from 'svelte';
  import { actionsFor } from '../lib/actions';
  import Icon from '../lib/Icon.svelte';
  import { app } from '../lib/state.svelte';

  const item = $derived(app.menu ? app.items[app.menu.id] : null);
  const groups = $derived(item ? actionsFor(item) : []);
  let menu = $state<HTMLDivElement>();
  let pos = $state({ x: 0, y: 0 });

  // Open at the pointer, kept inside the window, with the first action focused.
  $effect(() => {
    const m = app.menu;
    if (!m || !menu) return;
    const r = menu.getBoundingClientRect();
    pos = { x: Math.max(6, Math.min(m.x, innerWidth - r.width - 6)), y: Math.max(6, Math.min(m.y, innerHeight - r.height - 6)) };
    tick().then(() => menu?.querySelector<HTMLButtonElement>('button')?.focus());
  });

  function close() {
    app.menu = null;
  }
  async function pick(fn: () => unknown) {
    close();
    await fn();
  }
  function onKey(e: KeyboardEvent) {
    const all = [...(menu?.querySelectorAll<HTMLButtonElement>('button') ?? [])];
    const i = all.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === 'Escape' || e.key === 'Tab') {
      e.preventDefault();
      e.stopPropagation();
      close();
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      e.stopPropagation();
      all[(i + (e.key === 'ArrowDown' ? 1 : all.length - 1)) % all.length]?.focus();
    } else if (e.key === 'Home' || e.key === 'End') {
      e.preventDefault();
      all[e.key === 'Home' ? 0 : all.length - 1]?.focus();
    }
  }
</script>

<svelte:window onblur={close} onresize={close} />

{#if item}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="scrim" onpointerdown={close} oncontextmenu={(e) => (e.preventDefault(), close())}></div>
  <div class="menu" bind:this={menu} role="menu" aria-label="Actions for {item.fileName}" tabindex="-1" style:left="{pos.x}px" style:top="{pos.y}px" onkeydown={onKey}>
    <div class="head" title={item.fileName}>{item.fileName}</div>
    {#each groups as g, gi (gi)}
      {#if gi > 0}<div class="sep" role="separator"></div>{/if}
      {#each g as a (a.id)}
        <button role="menuitem" class:danger={a.danger} onclick={() => pick(a.run)}>
          <Icon name={a.icon} size={14} />
          <span class="l">{a.label}</span>
          {#if a.kbd}<kbd>{a.kbd}</kbd>{/if}
        </button>
      {/each}
    {/each}
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
  }
  .menu {
    position: fixed;
    z-index: 41;
    min-width: 232px;
    max-width: 320px;
    padding: 5px;
    border-radius: 9px;
    background: #1a1f25;
    border: 1px solid var(--line-strong);
    box-shadow: 0 14px 40px rgb(0 0 0 / 0.5);
    display: flex;
    flex-direction: column;
    animation: pop 0.12s ease-out;
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(-3px);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .menu {
      animation: none;
    }
  }
  .head {
    padding: 6px 9px 7px;
    font-family: var(--mono);
    font-stretch: 75%;
    font-size: 11.5px;
    color: var(--fog);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  button {
    display: flex;
    align-items: center;
    gap: 9px;
    height: 30px;
    padding: 0 9px;
    border-radius: 6px;
    text-align: left;
    color: var(--ivory);
  }
  button :global(svg) {
    flex: none;
    color: var(--fog);
  }
  button:hover,
  button:focus-visible {
    background: rgb(255 255 255 / 0.08);
    outline: none;
  }
  .l {
    flex: 1;
    white-space: nowrap;
  }
  .danger,
  .danger :global(svg) {
    color: var(--bad);
  }
  .danger:hover,
  .danger:focus-visible {
    background: rgb(255 107 107 / 0.12);
  }
  kbd {
    font-family: var(--mono);
    font-stretch: 75%;
    font-size: 10px;
    padding: 1px 4px;
    border-radius: 3px;
    border: 1px solid currentColor;
    opacity: 0.5;
  }
  .sep {
    height: 1px;
    margin: 4px 6px;
    background: var(--line-strong);
  }
</style>
