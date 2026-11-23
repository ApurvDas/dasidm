<script lang="ts">
  import type { Status } from './types';
  // State as a mark in a fixed cell: same shape everywhere it appears.
  let { status }: { status: Status } = $props();
  const label: Record<Status, string> = {
    queued: 'Queued',
    downloading: 'Loading',
    paused: 'Paused',
    done: 'Finished',
    failed: 'Failed',
  };
</script>

<span class="mark {status}" role="img" aria-label={label[status]}></span>

<style>
  .mark {
    flex: none;
    width: 10px;
    height: 10px;
    border-radius: 2px;
    display: inline-block;
  }
  .queued {
    box-shadow: inset 0 0 0 1.5px var(--fog);
  }
  .downloading {
    background: var(--ivory);
  }
  .paused {
    background: linear-gradient(90deg, var(--fog) 0 35%, transparent 35% 65%, var(--fog) 65%);
  }
  .done {
    background: var(--ok);
  }
  .failed {
    background: linear-gradient(135deg, transparent 40%, var(--ink) 40% 60%, transparent 60%), var(--bad);
  }
</style>
