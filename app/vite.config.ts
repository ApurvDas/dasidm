import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: {
    target: 'chrome120',
    cssMinify: true,
    assetsInlineLimit: 0,
    modulePreload: false,
  },
});
