<script lang="ts">
  // One stroke family, 16px grid, 1.5px lines.
  const paths: Record<string, string> = {
    plus: 'M8 3v10M3 8h10',
    pause: 'M5.5 3.5v9M10.5 3.5v9',
    play: 'M5 3.5l7 4.5-7 4.5z',
    stop: 'M4 4h8v8H4z',
    close: 'M4 4l8 8M12 4l-8 8',
    min: 'M3.5 8.5h9',
    max: 'M4 4h8v8H4z',
    folder: 'M2 4.5h4l1.5 1.5H14v6.5H2z',
    open: 'M9 3h4v4M13 3L7.5 8.5M11 9.5V13H3V5h3.5',
    trash: 'M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.7 8.5h5.6l.7-8.5',
    restart: 'M3.5 8a4.5 4.5 0 1 0 1.3-3.2M3.5 3v2.5H6',
    gear: 'M8 5.6a2.4 2.4 0 1 0 0 4.8 2.4 2.4 0 0 0 0-4.8zM8 1.8v1.6M8 12.6v1.6M1.8 8h1.6M12.6 8h1.6M3.6 3.6l1.1 1.1M11.3 11.3l1.1 1.1M3.6 12.4l1.1-1.1M11.3 4.7l1.1-1.1',
    search: 'M7 3a4 4 0 1 0 0 8 4 4 0 0 0 0-8zM10 10l3.5 3.5',
    up: 'M8 12.5v-9M4.5 7L8 3.5 11.5 7',
    caret: 'M4.5 10L8 6.5 11.5 10',
    right: 'M2.5 8h11M9.5 4l4 4-4 4',
    left: 'M13.5 8h-11M6.5 4l-4 4 4 4',
    down: 'M8 3.5v9M4.5 9L8 12.5 11.5 9',
    link: 'M7 9a2.8 2.8 0 0 0 4 0l2-2a2.8 2.8 0 0 0-4-4l-.8.8M9 7a2.8 2.8 0 0 0-4 0L3 9a2.8 2.8 0 0 0 4 4l.8-.8',
    check: 'M3.5 8.5l3 3 6-7',
    alert: 'M8 2.5l6 11H2zM8 6.5v3.5M8 11.8v.2',
  };
  let { name, size = 16 }: { name: string; size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  <path d={paths[name]} />
</svg>
