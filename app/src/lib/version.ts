// The app's version, from package.json (kept equal to tauri.conf.json's), set at build time in
// vite.config.ts.
declare const __APP_VERSION__: string;
export const version: string = __APP_VERSION__;
