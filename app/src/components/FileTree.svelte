<script lang="ts">
  // A torrent's contents as folders and files, like Explorer: folders fold open and shut, a
  // folder's box picks everything inside it, and while it downloads each line fills up.
  import { bytes } from '../lib/format';
  import Icon from '../lib/Icon.svelte';
  import { categorize } from '../lib/media';
  import { typeIcon, type TorrentFile } from '../lib/types';

  interface Folder {
    key: string;
    name: string;
    folders: Folder[];
    files: TorrentFile[];
    /** Every file inside, however deep. */
    all: TorrentFile[];
  }

  let {
    files,
    root = '',
    chosen = $bindable(),
    editable = false,
    progress = false,
    showRoot = false,
    folded = false,
  }: {
    files: TorrentFile[];
    /** The torrent's name: the folder its files land in. */
    root?: string;
    /** Files being fetched; the rest read as skipped. Without it, every file is. */
    chosen?: Set<number>;
    /** Checkboxes to include or skip files and whole folders. */
    editable?: boolean;
    /** A fill bar and percentage per line. */
    progress?: boolean;
    /** Show the torrent's own folder as the top line (where the files land). */
    showRoot?: boolean;
    /** Start with every folder shut (a short list where space is tight). */
    folded?: boolean;
  } = $props();

  const tree = $derived(build(files));

  function build(list: TorrentFile[]): Folder {
    const top: Folder = { key: '', name: '', folders: [], files: [], all: [] };
    for (const f of list) {
      let parts = f.path.split(/[\\/]/).filter(Boolean);
      // A multi-file torrent's paths all start with its own folder; that is the top line.
      if (!showRoot && list.length > 1 && parts.length > 1 && parts[0] === root) parts = parts.slice(1);
      parts.pop();
      let node = top;
      node.all.push(f);
      let key = '';
      for (const p of parts) {
        key += p + '\\';
        let next = node.folders.find((d) => d.name === p);
        if (!next) {
          next = { key, name: p, folders: [], files: [], all: [] };
          node.folders.push(next);
        }
        node = next;
        node.all.push(f);
      }
      node.files.push(f);
    }
    return top;
  }

  // Big torrents open with their sub-folders shut, so the top level reads at a glance.
  let closed = $state<Set<string> | null>(null);
  const shut = $derived(closed ?? firstShut(tree));
  function firstShut(t: Folder) {
    const s = new Set<string>();
    if (!folded && t.all.length <= 40) return s;
    const walk = (d: Folder, depth: number) => {
      for (const c of d.folders) {
        if (folded || depth >= (showRoot ? 1 : 0)) s.add(c.key);
        walk(c, depth + 1);
      }
    };
    walk(t, 0);
    return s;
  }
  function flip(key: string) {
    const s = new Set(shut);
    if (s.has(key)) s.delete(key);
    else s.add(key);
    closed = s;
  }

  const isOn = (f: TorrentFile) => !chosen || chosen.has(f.index);
  function setMany(list: TorrentFile[], on: boolean) {
    const next = new Set(chosen);
    for (const f of list) {
      if (on) next.add(f.index);
      else next.delete(f.index);
    }
    chosen = next;
  }
  /** Size and arrived bytes of the files being fetched (all of them when none are). */
  function sums(list: TorrentFile[]) {
    const pick = list.filter(isOn);
    const use = pick.length ? pick : list;
    return { size: use.reduce((a, f) => a + f.size, 0), done: use.reduce((a, f) => a + Math.min(f.done, f.size), 0), on: pick.length };
  }
  const pct = (done: number, size: number) => (size ? Math.floor((done / size) * 100) : 100);
  const leaf = (path: string) => path.split(/[\\/]/).pop() ?? path;

  /** The torrent's main video: the biggest one. Its level lists files before folders, with it
   * on top, so the episode or film is never hidden under a folder of extras. */
  const main = $derived.by(() => {
    let best: TorrentFile | null = null;
    for (const f of files) if (categorize(leaf(f.path)) === 'video' && (!best || f.size > best.size)) best = f;
    return best;
  });
  const filesOf = (node: Folder) => {
    const m = main;
    return m && node.files.includes(m) ? [m, ...node.files.filter((f) => f !== m)] : node.files;
  };

  /** Long names keep their ending (e.g. "…Scr 01.png") so the kind of file always shows. */
  function ends(name: string) {
    const dot = name.lastIndexOf('.');
    if (name.length <= 28) return { head: name, tail: '' };
    let at = name.length - Math.min(name.length, (dot > 0 ? name.length - dot : 0) + 10);
    // Start the ending at a word ("Screenshot 01.png", not "eenshot 01.png").
    const word = Math.max(name.lastIndexOf(' ', at), name.lastIndexOf('-', at), name.lastIndexOf('_', at));
    if (word > 0 && at - word <= 12) at = word + 1;
    return { head: name.slice(0, at), tail: name.slice(at) };
  }
</script>

{#snippet meter(done: number, size: number, skipped: boolean)}
  {#if skipped}
    <span class="track none"></span><span class="pc skip">Skipped</span>
  {:else}
    {@const v = pct(done, size)}
    <span class="track"><span class="fill" class:full={v >= 100} style:width="{v}%"></span></span>
    <span class="pc" class:full={v >= 100}>{#if v >= 100}<Icon name="check" size={12} />{:else}{v}%{/if}</span>
  {/if}
{/snippet}

{#snippet level(node: Folder, depth: number)}
  {#if main && node.files.includes(main)}
    {@render fileRows(node, depth)}
    {@render folderRows(node, depth)}
  {:else}
    {@render folderRows(node, depth)}
    {@render fileRows(node, depth)}
  {/if}
{/snippet}

{#snippet folderRows(node: Folder, depth: number)}
  {#each node.folders as d (d.key)}
    {@const open = !shut.has(d.key)}
    {@const s = sums(d.all)}
    {@const off = !!chosen && s.on === 0}
    <div class="row dir" class:off>
      <span class="ind" style:width="{depth * 18}px"></span>
      <button class="chev" class:open onclick={() => flip(d.key)} aria-expanded={open} aria-label="{open ? 'Close' : 'Open'} {d.name}"><Icon name="caret" size={12} /></button>
      {#if editable}
        <input
          type="checkbox"
          checked={s.on === d.all.length}
          indeterminate={s.on > 0 && s.on < d.all.length}
          onchange={() => setMany(d.all, s.on !== d.all.length)}
          aria-label="Download everything in {d.name}"
        />
      {/if}
      <span class="ico"><Icon name="folder" size={15} /></span>
      <span class="name" title={d.key.replace(/\\$/, '')}
        ><b>{d.name}</b><span class="cnt">{d.all.length} {d.all.length === 1 ? 'file' : 'files'}{#if chosen && s.on && s.on < d.all.length}, {s.on} chosen{/if}</span></span
      >
      {#if progress}{@render meter(s.done, s.size, off)}{/if}
      <span class="size">{bytes(s.size)}</span>
    </div>
    {#if open}{@render level(d, depth + 1)}{/if}
  {/each}
{/snippet}

{#snippet fileRows(node: Folder, depth: number)}
  {#each filesOf(node) as f (f.index)}
    {@const name = leaf(f.path)}
    {@const n = ends(name)}
    {@const off = !isOn(f)}
    <label class="row" class:off class:pick={editable} class:main={f === main}>
      <span class="ind" style:width="{depth * 18}px"></span>
      <span class="chev"></span>
      {#if editable}
        <input type="checkbox" checked={!off} onchange={() => setMany([f], off)} />
      {/if}
      <span class="ico"><Icon name={typeIcon(categorize(name))} size={14} /></span>
      <span class="name split" title={f.path}><span class="h">{n.head}</span>{#if n.tail}<span class="t">{n.tail}</span>{/if}</span>
      {#if progress}{@render meter(f.done, f.size, off)}{/if}
      <span class="size">{bytes(f.size)}</span>
    </label>
  {/each}
{/snippet}

<div class="tree" role="group" aria-label="Files in this torrent">
  {@render level(tree, 0)}
</div>

<style>
  .tree {
    display: flex;
    flex-direction: column;
    container-type: inline-size;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 32px;
    padding: 0 12px 0 6px;
    border-bottom: 1px solid var(--line);
  }
  .row:last-child {
    border-bottom: 0;
  }
  .row.pick {
    cursor: pointer;
  }
  .row:hover {
    background: var(--hover);
  }
  .ind {
    flex: none;
  }
  .chev {
    flex: none;
    width: 18px;
    height: 18px;
    display: grid;
    place-items: center;
    color: var(--fog);
    border-radius: 4px;
    transform: rotate(90deg);
    transition: transform 0.14s ease-out;
  }
  button.chev:hover {
    color: var(--ivory);
    background: var(--hover);
  }
  .chev.open {
    transform: rotate(180deg);
  }
  span.chev {
    transform: none;
  }
  @media (prefers-reduced-motion: reduce) {
    .chev {
      transition: none;
    }
  }
  input[type='checkbox'] {
    accent-color: var(--ivory);
    width: 14px;
    height: 14px;
    flex: none;
    margin: 0;
  }
  .ico {
    flex: none;
    display: grid;
    color: var(--fog);
  }
  .dir .ico {
    color: var(--ivory);
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name b {
    font-weight: 600;
  }
  /* The start of a long name gives way; its ending (number and extension) stays. */
  .name.split {
    display: flex;
    min-width: 0;
  }
  .name.split .h {
    flex: 0 1000 auto;
    min-width: 2ch;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: pre;
  }
  .name.split .t {
    flex: 0 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: pre;
  }
  .main .name {
    font-weight: 600;
  }
  .cnt {
    margin-left: 8px;
    font-size: 12px;
    color: var(--fog);
  }
  .track {
    flex: none;
    width: 84px;
    height: 6px;
    border-radius: 3px;
    background: rgb(255 255 255 / 0.07);
    overflow: hidden;
  }
  .track.none {
    background: none;
  }
  .fill {
    display: block;
    height: 100%;
    background: rgb(242 242 238 / 0.82);
    transition: width 0.4s ease-out;
  }
  .fill.full {
    background: var(--ok);
  }
  .pc,
  .size {
    flex: none;
    font-family: var(--mono);
    font-stretch: 75%;
    font-size: 12px;
    text-align: right;
  }
  .pc {
    width: 44px;
    display: flex;
    justify-content: flex-end;
  }
  .pc.full {
    color: var(--ok);
  }
  .pc.skip {
    color: var(--steel);
    font-family: var(--body);
    font-stretch: normal;
    font-size: 11.5px;
  }
  .size {
    width: 70px;
    color: var(--fog);
  }
  .off,
  .off .ico,
  .off.dir .ico {
    color: var(--steel);
  }
  @container (max-width: 380px) {
    .track {
      display: none;
    }
    .size {
      width: 60px;
    }
    .cnt {
      display: none;
    }
  }
</style>
