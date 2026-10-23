// Shared constants mirrored from the original single-file frontend.
export const ENV_LLAMA = 'LLAMA_CPP_PATH';
export const ENV_MODELS = 'LLAMA_CPP_MODELS_PATH';

export const TOAST_ICONS: Record<ToastType, string> = {
  success: '✔',
  info: 'ℹ',
  warning: '⚠',
  error: '✖',
};

export const MAX_TOASTS = 5;

export const DEFAULT_WEB_PORT = 8080;
/** Must stay in sync with `MIN_WEB_PORT` in src-tauri/src/config.rs. */
export const MIN_WEB_PORT = 1024;
export const MAX_WEB_PORT = 65535;

export type ToastType = 'success' | 'info' | 'warning' | 'error';
