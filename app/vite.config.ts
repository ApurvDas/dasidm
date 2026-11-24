import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { readFileSync } from 'node:fs';

// The app's version (kept equal to tauri.conf.json's), built into the page for the UI.
const { version } = JSON.parse(readFileSync(new URL('./package.json', import.meta.url), 'utf8'));

export default defineConfig({
  plugins: [svelte()],
  define: { __APP_VERSION__: JSON.stringify(version) },
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: {
    target: 'chrome120',
    cssMinify: true,
    assetsInlineLimit: 0,
    modulePreload: false,
  },
});
