import { reactive, ref } from 'vue';
import { cmd } from '../lib/tauri';
import { hasTauri } from '../lib/hasTauri';
import { ENV_LLAMA, ENV_MODELS } from '../lib/const';
import { useToast } from './useToast';

// ── Shared reactive state (mirrors the original single setup()) ──
export const env = reactive({
  llamaCppPath: '',
  llamaValid: false,
  modelsPath: '',
  modelsValid: false,
});
export const execPath = ref('');
export const models = ref<string[]>([]);
export const modelPath = ref('');
export const scanning = ref(false);
export const scanMsg = ref('');

/** Read env vars from backend and refresh derived state. */
export async function refreshEnv() {
  if (!hasTauri()) return;
  try {
    const s = await cmd.get_env_settings();
    env.llamaCppPath = s.llama_cpp_path || '';
    env.llamaValid = !!s.llama_valid;
    env.modelsPath = s.models_path || '';
    env.modelsValid = !!s.models_valid;
    // The backend resolves the platform-specific executable path, so the UI
    // can never show a stale/wrong path (e.g. `.exe` on Unix).
    execPath.value = s.exec_path || '';
  } catch (err) {
    useToast().showToast('读取设置失败: ' + err, 'error');
  }
}

async function saveLlamaPath() {
  if (!hasTauri()) return;
  if (!env.llamaCppPath.trim()) {
    useToast().showToast('请输入 llama.cpp 目录路径', 'error');
    return;
  }
  try {
    await cmd.save_env_var(ENV_LLAMA, env.llamaCppPath.trim());
    await refreshEnv();
    useToast().showToast('已保存 LLAMA_CPP_PATH', 'success');
    if (env.llamaValid) {
      // llama.cpp archives ship with a sibling `models` folder; offering it
      // saves the user a browse dialog. It is only filled in, not saved.
      if (!env.modelsPath.trim()) await detectModelsPath();
      loadModels();
    }
  } catch (err) {
    useToast().showToast('保存失败: ' + err, 'error', 5000);
  }
}

/** Fill the models input with `<llama.cpp>/models` when that folder exists. */
async function detectModelsPath(): Promise<boolean> {
  if (!hasTauri() || !env.llamaCppPath.trim()) return false;
  try {
    const found = await cmd.suggest_models_path(env.llamaCppPath.trim());
    if (!found) {
      useToast().showToast('llama.cpp 目录下没有 models 文件夹', 'warning');
      return false;
    }
    env.modelsPath = found;
    useToast().showToast('已填入 models 目录，点击保存后生效', 'info', 4000);
    return true;
  } catch (err) {
    useToast().showToast('检测 models 目录失败: ' + err, 'error');
    return false;
  }
}

async function saveModelsPath() {
  if (!hasTauri()) return;
  if (!env.modelsPath.trim()) {
    useToast().showToast('请输入 models 目录路径', 'error');
    return;
  }
  try {
    await cmd.save_env_var(ENV_MODELS, env.modelsPath.trim());
    await refreshEnv();
    useToast().showToast('已保存 LLAMA_CPP_MODELS_PATH', 'success');
    loadModels();
  } catch (err) {
    useToast().showToast('保存失败: ' + err, 'error', 5000);
  }
}

async function browseFolder(target: 'llama' | 'models') {
  if (!hasTauri()) {
    useToast().showToast('文件浏览仅支持打包后的 Tauri 桌面程序', 'error', 4000);
    return;
  }
  try {
    const selected = await cmd.pick_folder();
    if (!selected) return;
    if (target === 'llama') {
      env.llamaCppPath = selected;
      await saveLlamaPath();
    } else {
      env.modelsPath = selected;
      await saveModelsPath();
    }
  } catch (err) {
    useToast().showToast('操作失败: ' + err, 'error');
  }
}

export async function loadModels() {
  if (!hasTauri()) return;
  try {
    const list = await cmd.list_models();
    const prev = modelPath.value;
    models.value = list;
    if (list.includes(prev)) {
      modelPath.value = prev;
    } else if (list.length > 0) {
      modelPath.value = list[0];
    } else {
      modelPath.value = '';
    }
  } catch (err) {
    useToast().showToast('读取模型失败: ' + err, 'error');
  }
}

/** User-triggered refresh: same scan with explicit feedback. */
export async function refreshModels() {
  await loadModels();
  useToast().showToast(`已刷新，共 ${models.value.length} 个模型`, 'success', 2500);
}

export function useEnvModels() {
  return {
    env,
    execPath,
    models,
    modelPath,
    scanning,
    scanMsg,
    refreshEnv,
    saveLlamaPath,
    saveModelsPath,
    browseFolder,
    detectModelsPath,
    loadModels,
    refreshModels,
  };
}
