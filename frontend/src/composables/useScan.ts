import { ref, onUnmounted } from 'vue';
import { cmd, subscribe } from '../lib/tauri';
import { hasTauri } from '../lib/hasTauri';
import { ENV_LLAMA } from '../lib/const';
import { useToast } from './useToast';
import { scanning, scanMsg, loadModels, refreshEnv } from './useEnvModels';

function basename(p: string) {
  return String(p).split(/[\\/]/).pop() as string;
}

/** Candidate directories found by the last scan. The user picks one; a scan
 * never writes LLAMA_CPP_PATH on its own. */
export const scanResults = ref<string[]>([]);

export function useScan() {
  const unlistenScanMsg = ref<(() => void) | null>(null);

  async function startScan() {
    if (!hasTauri()) return;
    if (!unlistenScanMsg.value) {
      unlistenScanMsg.value = await subscribe<string>('scan-message', (msg) => {
        scanMsg.value = msg;
      });
    }
    scanning.value = true;
    scanResults.value = [];
    scanMsg.value = '准备扫描…';
    try {
      const results = await cmd.scan_llama_cpp();
      if (results.length === 0) {
        scanMsg.value =
          '未找到 llama.cpp 目录。请在下方「环境变量」中设置 LLAMA_CPP_PATH，或检查 llama-server 是否在 PATH 中。';
      } else {
        scanResults.value = results;
        scanMsg.value =
          results.length === 1
            ? '找到 1 个候选目录，请确认后使用'
            : `找到 ${results.length} 个候选目录，请选择要使用的目录`;
      }
    } catch (err) {
      scanMsg.value = '扫描失败: ' + err;
    } finally {
      scanning.value = false;
    }
  }

  /** Apply a scan candidate (explicit user action). */
  async function applyCandidate(path: string) {
    if (!hasTauri()) return;
    try {
      await cmd.save_env_var(ENV_LLAMA, path);
      await refreshEnv();
      await loadModels();
      scanResults.value = [];
      scanMsg.value = '';
      useToast().showToast('已设置 llama.cpp: ' + basename(path), 'success');
    } catch (err) {
      useToast().showToast('保存失败: ' + err, 'error');
    }
  }

  async function cancelScan() {
    if (!hasTauri()) return;
    try {
      await cmd.cancel_scan();
      scanning.value = false;
      scanResults.value = [];
      scanMsg.value = '已取消扫描';
    } catch (err) {
      useToast().showToast('取消失败: ' + err, 'error');
    }
  }

  onUnmounted(() => {
    unlistenScanMsg.value?.();
  });

  return { startScan, cancelScan, applyCandidate, scanResults };
}
