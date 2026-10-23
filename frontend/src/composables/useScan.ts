import { ref, onUnmounted } from 'vue';
import { cmd, subscribe } from '../lib/tauri';
import { hasTauri } from '../lib/hasTauri';
import { ENV_LLAMA } from '../lib/const';
import { useToast } from './useToast';
import { scanning, scanMsg, loadModels, refreshEnv } from './useEnvModels';

function basename(p: string) {
  return String(p).split(/[\\/]/).pop() as string;
}

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
    scanMsg.value = '准备扫描…';
    try {
      const results = await cmd.scan_llama_cpp();
      if (results.length === 0) {
        scanMsg.value =
          '未找到 llama.cpp 目录。请在上方设置 LLAMA_CPP_PATH 环境变量，或检查 llama-server.exe 是否在 PATH 中。';
      } else {
        const chosen = results[0];
        // Auto-scan writes the chosen path directly (no input-field guard).
        await cmd.save_env_var(ENV_LLAMA, chosen);
        await refreshEnv();
        loadModels();
        scanMsg.value = '';
        useToast().showToast('已识别 llama.cpp: ' + basename(chosen), 'success');
      }
    } catch (err) {
      scanMsg.value = '扫描失败: ' + err;
    } finally {
      scanning.value = false;
    }
  }

  async function cancelScan() {
    if (!hasTauri()) return;
    try {
      await cmd.cancel_scan();
      scanning.value = false;
      scanMsg.value = '已取消扫描';
    } catch (err) {
      useToast().showToast('取消失败: ' + err, 'error');
    }
  }

  onUnmounted(() => {
    unlistenScanMsg.value?.();
  });

  return { startScan, cancelScan };
}
