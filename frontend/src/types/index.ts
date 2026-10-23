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
  /** Platform-resolved llama-server path; `null` when not valid. */
  exec_path: string | null;
  models_path: string | null;
  models_valid: boolean;
}

/** `get_status` return value: [running, ready, pid, port]. */
export type StatusSnapshot = [boolean, boolean, number | null, number];

/**
 * `get_status_full` return value — richer status used by the API panel,
 * including the active model alias and api_key.
 */
export interface StatusFull {
  running: boolean;
  ready: boolean;
  pid: number | null;
  port: number;
  model_alias: string;
  api_key: string | null;
}

/** Advanced launch parameters, shared across all models. */
export interface AdvancedParams {
  ctx_size: number | null;
  ngl: number | null;
  /** Free-form extra CLI args, tokenized with quote support. */
  extra_args: string | null;
  api_key: string | null;
}

/** `check_config` return value. */
export interface ConfigInfo {
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
  ready: boolean;
  pid: number | null;
  port: number;
}

/** `process-exit` event payload: the process exited on its own. */
export interface ProcessExitEvent {
  code: number | null;
  success: boolean;
}

/** `scan-done` event payload. */
export interface ScanDoneEvent {
  count: number;
  paths: string[];
}
