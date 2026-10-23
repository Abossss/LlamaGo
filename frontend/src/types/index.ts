// TypeScript shapes for the Tauri backend contract.
// These mirror the commands and events declared in src-tauri/src/main.rs.
// Keep in sync when the backend changes.

/** Payload of the `toast` event. */
export interface ToastPayload {
  message: string;
  type?: string;
}

/** `get_env_settings` return value. */
export interface EnvSettings {
  llama_cpp_path: string | null;
  llama_valid: boolean;
  models_path: string | null;
  models_valid: boolean;
}

/** `get_status` return value: [running, pid, port]. */
export type StatusTriple = [boolean, number | null, number];

/**
 * `get_status_full` return value — richer status used by the API panel,
 * including the active model alias and api_key.
 */
export interface StatusFull {
  running: boolean;
  pid: number | null;
  port: number;
  model_alias: string;
  api_key: string | null;
}

/** Advanced per-model parameters surfaced in the UI. */
export interface AdvancedParams {
  ctx_size: number | null;
  ngl: number | null;
  temperature: number | null;
  max_tokens: number | null;
  api_key: string | null;
}

/** `check_config` return value. */
export interface ConfigInfo {
  exec_path: string | null;
  model_path: string | null;
  config_path: string;
  // New in the advanced-params feature.
  model_aliases: Record<string, string>;
  advanced: AdvancedParams | null;
  current_alias: string | null;
  /** Configured Web UI port; `null` = llama.cpp's default. */
  web_port: number | null;
  /** llama-server's own default port (detected from `--help`), if resolvable. */
  server_default_web_port: number | null;
}

/** `server-status` event payload. */
export interface ServerStatusEvent {
  running: boolean;
  pid: number | null;
  port: number;
}

/** `scan-done` event payload. */
export interface ScanDoneEvent {
  count: number;
  paths: string[];
}
