# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

LlamaGo is a Tauri 2.x desktop launcher for llama.cpp's `llama-server`: a desktop GUI that manages the full lifecycle of the inference server (scan for `llama-server` executable and `.gguf` models, start/stop, live logs, system tray, single-instance protection). UI text is in Chinese; user-facing strings and toasts are Chinese throughout.

- **Backend**: `src-tauri/src/` — split into focused modules: `state.rs` (shared `Mutex<AppState>` + constants), `config.rs` (persisted `config.json`), `env.rs` (env-var resolution/persistence), `cmd_config.rs` / `cmd_env.rs` / `misc.rs` (thin command wrappers), `scan.rs` / `models.rs` (filesystem discovery), `server.rs` (llama-server spawn/stop/log relay/exit watcher + status commands), `tray.rs` (system tray + shared emit helpers), all wired together in a slim `main.rs` (single-instance guard, Tauri state, `generate_handler!`).
- **Frontend**: `frontend/` — Vue 3 SFC + TypeScript, Vite dev server on port 5173 (matches `devUrl` in `src-tauri/tauri.conf.json`). No router; `App.vue` switches between `LaunchView` and `SettingsView` via a `tab` ref.
- README (root `README.md`) is the primary source of truth for behavior: it documents the full command/event list, path-resolution priority, stop strategies, tray menu semantics, shortcuts, and known limitations.

## Build & run

```sh
cd frontend && npm install

# frontend dev server only (port 5173)
cd frontend && npm run dev

# full desktop app in dev mode (Tauri CLI auto-runs beforeDevCommand -> Vite)
cd src-tauri && cargo tauri dev

# production build: runs `npm run build` (vue-tsc --noEmit && vite build) into
# frontend/dist, then bundles. Installers land in target/release/bundle/
cd src-tauri && cargo tauri build
```

- `npm run build` = `vue-tsc --noEmit && vite build` — **vue-tsc is the only frontend lint**; there is no frontend test runner.
- CI (`.github/workflows/build.yml`) runs `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` and the frontend typecheck; release builds run on `v*` tags across windows-latest, ubuntu-22.04, and both macOS arches via `tauri-action`.
- Rust unit tests live in `#[cfg(test)] mod tests` inside each backend module (config, server, models); run them with `cargo test` in `src-tauri`.

## Architecture

### Frontend <-> backend boundary (critical)

`frontend/src/lib/tauri.ts` is the **only** place the frontend talks to Rust: a typed `cmd` object wrapping every `invoke` command (types mirror `main.rs` signatures in `frontend/src/types/index.ts`) plus a `subscribe(name, handler)` helper returning an unlisten fn. **Always route new IPC through `cmd` in `tauri.ts` and add the event's payload type in `types/index.ts`** — do not call `invoke` from components directly. `frontend/src/lib/hasTauri.ts` guards the browser-only dev mode (Vite without Tauri), so every IPC call site must `hasTauri()`-check before invoking.

### Shared state via module-level composables (not provide/inject)

State is split into `frontend/src/composables/*`, each exporting **module-level** refs/functions (deliberately: components import and share the same instances; there is no global store). Key ones:

- `useServer.ts` — `running` / `starting` / `ready` / `pid` / `webPort` / `logs` refs, running-model metadata (`serverModelId` / `activeAlias` / `activeKey`), `launchArgs` / `launchCommand` preview, log-follow state, `syncStatus()`, `startServer()` / `stopServer()` / `restartServer()`, and `bindServerEvents()` (subscribes to `log-line`, `server-status`, `server-ready`, `process-exit` and backfills via `get_log_history`). Bound once at root in `App.vue` (never unlistened, since the app is single-window).
- `useEnvModels.ts` — env-var settings + scanned model list; `modelPath` ref holds the current selection; `refreshModels()` / `detectModelsPath()` helpers.
- `useConfig.ts` / `useAdvanced.ts` / `useScan.ts` / `useToast.ts` — last-selection restore, global advanced params (alias/api_key/ctx/ngl/extra_args, debounced persistence), llama.cpp dir scan (candidates require explicit confirmation), global toasts.
- Note in `useServer.ts` header: state was intentionally hoisted to module scope so `Sidebar` and `LaunchView` observe the same refs — don't re-scope per component.

### Backend module layout

- `state.rs` — `AppState` (behind `std::sync::Mutex`, managed as `tauri::State`) holds the spawned `Child`, `is_running` / `is_ready` flags, PID, port, running model metadata, and a 200-line `VecDeque` log ring buffer; `AppStateLock` is the `Mutex<AppState>` alias used at every call site.
- `config.rs` — `AppConfig` serialized to `config.json` under the app data dir (`get_config_dir` -> `app_data_dir()/LlamaGo/`); stores `model_path`, per-model `model_aliases`, `web_port`, and global `advanced` params. The executable is always derived from `LLAMA_CPP_PATH`, never persisted. `AdvancedParams` holds `ctx_size`, `ngl`, `extra_args` (free-form, tokenized with quote support) and `api_key`; generation params such as temperature/max tokens are set per request by OpenAI clients.
- `env.rs` — `LLAMA_CPP_PATH`/`LLAMA_CPP_MODELS_PATH` resolution (process env first, then the platform-specific persisted store: `setx`/HKCU on Windows, a `key=value` file under `~/.config/llamago/env` on Unix) + `server_exe_name()` / `basename_of()` helpers. The frontend cannot override the resolved executable path — it's derived server-side (security note in README).
- `server.rs` — llama-server spawn, stdout/stderr relay threads (readiness detected from `server is listening`), exit watcher (emits `process-exit` with `{code, success}` only when the user did not stop it), `terminate_child` (Windows: `TerminateProcess`; Unix: SIGTERM→SIGKILL after 5s, gated by `#[cfg]`), plus `start_server`/`stop_server`/`get_status`/`get_status_full`/`get_log_history` and the tray-facing `start_server_from_tray`/`stop_server_now`/`web_url` helpers.
- `tray.rs` — system-tray menu build/handlers (`toggle_server_from_tray`, `update_tray_menu`, …) and the shared emit helpers (`emit_toast`, `emit_server_status`, `write_log` → `debug.log` + `log-line`).
- `scan.rs` / `models.rs` — `scan_llama_cpp`/`cancel_scan` (off-thread via `tokio::spawn_blocking`, cancellable with an `AtomicBool`) and `list_models` (recursive `.gguf` walk, depth-capped, canonical-path dedup against symlink loops; skips `mmproj*` and split-model continuation shards).
- `cmd_config.rs` / `cmd_env.rs` / `misc.rs` — thin `#[tauri::command]` wrappers (`check_config`/`save_config`; `get_env_settings`/`save_env_var`/`pick_folder`/`suggest_models_path`; `open_url`/`export_log`).
- `main.rs` — slim entry point only: `tauri-plugin-single-instance` (a second launch focuses the existing window), window-close interception (`WindowEvent::CloseRequested` -> `hide()` + `prevent_close`, so the app lives in the tray; only the tray "quit" item stops the server then exits), Tauri state registration, tray setup, and the full `generate_handler!` list.

## Conventions worth keeping

- Adding a new command: implement it in the most topical module (`server.rs`, `scan.rs`, …), register it in `main.rs`'s `generate_handler!` list, mirror its signature in `tauri.ts`'s `cmd` object, and add its payload/return types to `types/index.ts`. If it emits frontend-facing events, document the event name in README's 前端事件 table.
- New events follow the existing pattern: backend emits via `Emitter` (`app.emit("event-name", payload)`), frontend consumes via `subscribe('event-name', ...)` from `tauri.ts`.
- Frontend components are split by UI concern (`LaunchView`, `SettingsView`, `LogPanel`, `ApiPanel`, `AdvancedParams`, `Sidebar`, `Toasts`); when a component grows, split into a new file in `components/` rather than growing an existing one.
- All user-facing strings are Chinese — keep new UI text/labels/toasts in Chinese.
- CI builds 4 targets (Windows x86_64, Linux x86_64, macOS aarch64 + x86_64) — `Cargo.toml`'s `release` profile has `lto = true` and `codegen-units = 1`, so cross-arch CI builds are slow by design.
