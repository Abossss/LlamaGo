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

function serverExe(dir: string) {
  const sep = dir.endsWith('\\') || dir.endsWith('/') ? '' : '\\';
  return dir + sep + 'llama-server.exe';
}

/** Read env vars from backend and refresh derived state. */
export async function refreshEnv() {
  if (!hasTauri()) return;
  try {
    const s = await cmd.get_env_settings();
    env.llamaCppPath = s.llama_cpp_path || '';
    env.llamaValid = !!s.llama_valid;
    env.modelsPath = s.models_path || '';
    env.modelsValid = !!s.models_valid;
    if (env.llamaValid) {
      execPath.value = serverExe(env.llamaCppPath);
    }
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
    if (env.llamaValid) loadModels();
  } catch (err) {
    useToast().showToast('保存失败: ' + err, 'error', 5000);
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
    loadModels,
  };
}
