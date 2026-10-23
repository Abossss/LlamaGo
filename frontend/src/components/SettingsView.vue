<script setup lang="ts">
import { useEnvModels, env } from '../composables/useEnvModels';
import { useScan } from '../composables/useScan';
import AdvancedParams from './AdvancedParams.vue';
import ApiPanel from './ApiPanel.vue';

const {
  saveLlamaPath,
  saveModelsPath,
  browseFolder,
  detectModelsPath,
  scanning,
  scanMsg,
} = useEnvModels();
const { startScan, cancelScan, applyCandidate, scanResults } = useScan();

const baseName = (p: string) => p.split(/[\\/]/).pop() || p;

const sections = [
  { id: 'settings-env', label: '环境变量' },
  { id: 'settings-advanced', label: '高级参数' },
  { id: 'settings-api', label: 'API 调用' },
];

function scrollTo(id: string) {
  document.getElementById(id)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
}
</script>

<template>
  <div class="view settings-view">
    <header class="view-header">
      <p class="view-kicker">配置</p>
      <h2 class="view-title">设置</h2>
      <p class="view-sub">
        路径保存为用户环境变量（Windows 用 setx，其他平台写入用户配置），保存后立即生效
      </p>
    </header>

    <nav class="settings-nav" aria-label="设置分区">
      <a
        v-for="s in sections"
        :key="s.id"
        :href="'#' + s.id"
        @click.prevent="scrollTo(s.id)"
      >
        {{ s.label }}
      </a>
    </nav>

    <div
      v-if="!env.llamaValid || scanning || scanResults.length"
      class="banner"
      :class="scanning ? 'banner-info' : 'banner-warn'"
    >
      <span class="banner-icon">{{ scanning ? '⏳' : '◌' }}</span>
      <div class="banner-body">
        <div class="banner-title">
          {{
            scanning
              ? '正在自动检测 llama.cpp …'
              : scanResults.length
                ? `找到 ${scanResults.length} 个候选目录`
                : '未检测到 llama.cpp 目录'
          }}
        </div>
        <div v-if="scanning" class="banner-actions">
          <button class="btn btn-secondary btn-sm" @click="cancelScan">取消扫描</button>
        </div>
        <div v-else-if="scanResults.length">
          <div>请选择要使用的 llama.cpp 目录，选中后立即写入 LLAMA_CPP_PATH：</div>
          <ul class="scan-results">
            <li v-for="path in scanResults" :key="path">
              <button class="scan-candidate" @click="applyCandidate(path)">
                <span class="scan-candidate-name">{{ baseName(path) }}</span>
                <span class="scan-candidate-path">{{ path }}</span>
              </button>
            </li>
          </ul>
        </div>
        <div v-else>
          {{ scanMsg || '请手动扫描，或在下方「环境变量」中配置 LLAMA_CPP_PATH 环境变量。' }}
        </div>
      </div>
      <div v-if="!scanning" class="banner-actions">
        <button class="btn btn-secondary btn-sm" @click="startScan">
          {{ scanResults.length ? '重新扫描' : '自动扫描' }}
        </button>
      </div>
    </div>

    <section id="settings-env" class="panel settings-anchor">
      <div class="panel-header">
        <span class="panel-icon">⚙</span>
        <div>
          <h3 class="panel-title">环境变量</h3>
          <p class="panel-sub">llama.cpp 与 models 目录路径</p>
        </div>
      </div>

      <div class="env-grid">
        <div class="input-group">
          <label class="input-label" for="env-llama-path">
            llama.cpp 目录
            <span class="hint">LLAMA_CPP_PATH</span>
          </label>
          <div class="input-row">
            <input
              id="env-llama-path"
              type="text"
              v-model="env.llamaCppPath"
              placeholder="包含 llama-server 的目录 …"
              class="input-field"
              @keyup.enter="saveLlamaPath"
            />
            <button class="btn btn-secondary btn-sm" @click="browseFolder('llama')">浏览</button>
            <button class="btn btn-primary btn-sm" @click="saveLlamaPath">保存</button>
          </div>
          <div class="model-count">
            <span class="badge" :class="env.llamaValid ? 'badge-ok' : 'badge-bad'">
              {{ env.llamaValid ? '✓ 有效' : (env.llamaCppPath ? '✕ 无效' : '未配置') }}
            </span>
            <span v-if="scanning" style="margin-left: 8px">扫描中… {{ scanMsg }}</span>
          </div>
        </div>

        <div class="input-group">
          <label class="input-label" for="env-models-path">
            models 目录
            <span class="hint">LLAMA_CPP_MODELS_PATH</span>
          </label>
          <div class="input-row">
            <input
              id="env-models-path"
              type="text"
              v-model="env.modelsPath"
              placeholder="包含 .gguf 模型的目录 …"
              class="input-field"
              @keyup.enter="saveModelsPath"
            />
            <button
              class="btn btn-secondary btn-sm"
              title="使用 llama.cpp 目录下的 models 文件夹"
              :disabled="!env.llamaValid"
              @click="detectModelsPath"
            >
              自动识别
            </button>
            <button class="btn btn-secondary btn-sm" @click="browseFolder('models')">浏览</button>
            <button class="btn btn-primary btn-sm" @click="saveModelsPath">保存</button>
          </div>
          <div class="model-count">
            <span class="badge" :class="env.modelsValid ? 'badge-ok' : 'badge-bad'">
              {{ env.modelsValid ? '✓ 有效' : (env.modelsPath ? '✕ 无效' : '未配置') }}
            </span>
          </div>
        </div>
      </div>
    </section>

    <div id="settings-advanced" class="settings-anchor">
      <AdvancedParams />
    </div>

    <div id="settings-api" class="settings-anchor">
      <ApiPanel />
    </div>
  </div>
</template>

<style scoped>
.settings-nav {
  display: flex;
  gap: 6px;
  margin-bottom: 14px;
  flex-wrap: wrap;
}

.settings-nav a {
  font-size: 12.5px;
  color: var(--ink-faint);
  text-decoration: none;
  padding: 4px 10px;
  border-radius: var(--radius-pill);
  border: 1px solid var(--border-strong);
  background: var(--surface);
  transition: color 0.18s ease, border-color 0.18s ease;
}

.settings-nav a:hover,
.settings-nav a:focus-visible {
  color: var(--ink-soft);
  border-color: #c5c3ba;
}

.settings-anchor {
  scroll-margin-top: 20px;
}

.scan-results {
  list-style: none;
  margin: 8px 0 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.scan-candidate {
  display: flex;
  flex-direction: column;
  gap: 1px;
  width: 100%;
  text-align: left;
  background: var(--surface-elevated);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  padding: 7px 10px;
  cursor: pointer;
  font-family: var(--font-sans);
  transition: border-color 0.18s ease, background 0.18s ease;
}

.scan-candidate:hover {
  border-color: var(--brand);
  background: var(--brand-light);
}

.scan-candidate-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--ink);
}

.scan-candidate-path {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--ink-faint);
  word-break: break-all;
}
</style>
