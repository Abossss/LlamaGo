// Tauri availability guard — preserved from the original CDN build.
// Lets the page render and degrade gracefully when opened in a plain browser
// (no Tauri runtime). Tauri-only actions check this and show a toast.
export function hasTauri(): boolean {
  // @tauri-apps/api sets window.__TAURI_INTERNALS__ / __TAURI_IPC__ at runtime.
  return !!(window as any).__TAURI_INTERNALS__ || !!(window as any).__TAURI__;
}
