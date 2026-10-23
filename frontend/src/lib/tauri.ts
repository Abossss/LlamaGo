// Wrapper around @tauri-apps/api for the backend commands.
// The only place in the app that talks to the Rust backend.
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type {
  AdvancedParams,
  ConfigInfo,
  EnvSettings,
  ProcessExitEvent,
  ScanDoneEvent,
  ServerStatusEvent,
  StatusFull,
  StatusSnapshot,
  ToastPayload,
} from '../types';

// Re-export for convenience
export type {
  AdvancedParams,
  ConfigInfo,
  EnvSettings,
  ProcessExitEvent,
  ScanDoneEvent,
  ServerStatusEvent,
  StatusFull,
  StatusSnapshot,
  ToastPayload,
};

/** All backend commands. Argument/return types match main.rs. */
export const cmd = {
  // Tauri expects camelCase argument keys (the Rust macro defaults to
  // `rename_all = "camelCase"`), so map our snake_case-shaped API here.
  start_server: (
    args: string[],
    model_alias?: string | null,
    api_key?: string | null,
  ) =>
    invoke<string>('start_server', {
      args,
      modelAlias: model_alias,
      apiKey: api_key,
    }),
  stop_server: () => invoke<string>('stop_server'),
  get_status: () => invoke<StatusSnapshot>('get_status'),
  get_status_full: () => invoke<StatusFull>('get_status_full'),
  check_config: () => invoke<ConfigInfo>('check_config'),
  save_config: (
    model_path: string | null,
    model_aliases?: Record<string, string> | null,
    advanced?: AdvancedParams | null,
    web_port?: number | null,
  ) =>
    invoke<void>('save_config', {
      modelPath: model_path,
      modelAliases: model_aliases,
      advanced,
      webPort: web_port,
    }),
  get_env_settings: () => invoke<EnvSettings>('get_env_settings'),
  save_env_var: (name: string, value: string) =>
    invoke<void>('save_env_var', { name, value }),
  suggest_models_path: (llama_dir: string) =>
    invoke<string | null>('suggest_models_path', { llamaDir: llama_dir }),
  scan_llama_cpp: () => invoke<string[]>('scan_llama_cpp'),
  cancel_scan: () => invoke<void>('cancel_scan'),
  list_models: () => invoke<string[]>('list_models'),
  get_log_history: () => invoke<string[]>('get_log_history'),
  pick_folder: () => invoke<string | null>('pick_folder'),
  open_url: (url: string) => invoke<void>('open_url', { url }),
  export_log: (content: string, default_name: string) =>
    invoke<string | null>('export_log', { content, defaultName: default_name }),
};

/** Convenience alias for the event payload types used by subscribe helpers. */
export type {
  ProcessExitEvent as ProcessExit,
  ScanDoneEvent as ScanDone,
  ServerStatusEvent as ServerStatus,
  ToastPayload as ToastEvent,
};

/**
 * Subscribe to a backend event. Returns an `unlisten` function that must be
 * called on component teardown to avoid duplicate handlers.
 */
export function subscribe<T>(
  name: string,
  handler: (payload: T) => void,
): Promise<() => void> {
  return listen<T>(name, (event) => {
    handler(event.payload);
  });
}
