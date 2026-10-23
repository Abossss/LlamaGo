import { ref } from 'vue';
import { cmd } from '../lib/tauri';
import { hasTauri } from '../lib/hasTauri';
import { MAX_WEB_PORT, MIN_WEB_PORT } from '../lib/const';
import { modelPath, models } from './useEnvModels';
import { aliases, advanced, emptyAdvanced } from './useAdvanced';
import { useToast } from './useToast';

/** Configured Web UI port; `null` = llama.cpp's own default. */
export const webPort = ref<number | null>(null);

/** llama-server's own default port, detected by the backend from `--help`. */
export const serverDefaultWebPort = ref<number | null>(null);

/**
 * The saved config has been loaded into the module state. Until then the
 * startup auto-selection must never be written back: doing so would overwrite
 * the saved advanced params / port with in-memory defaults.
 */
let configHydrated = false;

/** Mirrors the backend's `validate_web_port` so invalid values never reach
 * `save_config` (and the user gets the same message either way). */
export function isValidWebPort(port: number | null): boolean {
  return (
    port == null ||
    (Number.isInteger(port) && port >= MIN_WEB_PORT && port <= MAX_WEB_PORT)
  );
}

export function webPortRangeMessage(): string {
  return `Web 端口必须在 ${MIN_WEB_PORT}–${MAX_WEB_PORT} 之间`;
}

export function useConfig() {
  async function restoreLastSelection() {
    if (!hasTauri()) return;
    try {
      const cfg = await cmd.check_config();
      // Only now is the in-memory state authoritative enough to persist.
      configHydrated = true;
      if (cfg.model_path && models.value.includes(cfg.model_path)) {
        modelPath.value = cfg.model_path;
      }
      // Pull in the persisted advanced params + aliases so the UI reflects
      // the last-used values.
      // Hydrate with defaults first: configs written before `extra_args` (or
      // with the removed temperature/max_tokens) must not leave holes.
      if (cfg.advanced) {
        advanced.value = { ...emptyAdvanced(), ...cfg.advanced };
      }
      if (cfg.model_aliases) {
        aliases.value = { ...aliases.value, ...cfg.model_aliases };
      }
      webPort.value = cfg.web_port ?? null;
      serverDefaultWebPort.value = cfg.server_default_web_port ?? null;
      // Persist the model the UI actually shows (auto-selected on first run),
      // so a tray-initiated start keeps using it. Values are hydrated by now.
      if (modelPath.value) await saveSelection();
    } catch (err) {
      // A corrupt config is now a hard backend error (with a backup file), so
      // surface it instead of silently running with defaults. Allow later
      // user edits to be saved over the backed-up file.
      configHydrated = true;
      useToast().showToast('读取配置失败: ' + err, 'error', 5000);
    }
  }

  async function saveSelection() {
    if (!hasTauri() || !configHydrated) return;
    if (!isValidWebPort(webPort.value)) {
      useToast().showToast(webPortRangeMessage(), 'error');
      return;
    }
    try {
      // Persist the model path, current aliases, advanced set, and web port.
      // The backend merges these into the existing config.json (a null model
      // path keeps the previously saved selection untouched), so the Web port
      // can also be saved before choosing a model.
      await cmd.save_config(
        modelPath.value || null,
        aliases.value,
        advanced.value,
        webPort.value,
      );
    } catch (err) {
      useToast().showToast('保存配置失败: ' + err, 'error');
    }
  }

  return { restoreLastSelection, saveSelection };
}
