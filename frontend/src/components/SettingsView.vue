<script setup lang="ts">
import { useEnvModels, env } from '../composables/useEnvModels';
import { useScan } from '../composables/useScan';
import AdvancedParams from './AdvancedParams.vue';
import ApiPanel from './ApiPanel.vue';

const {
  saveLlamaPath,
  saveModelsPath,
  browseFolder,
  scanning,
  scanMsg,
} = useEnvModels();
const { startScan, cancelScan } = useScan();
</script>

<template>
  <div class="view settings-view">
    <header class="view-header">
      <p class="view-kicker">配置</p>
      <h2 class="view-title">设置</h2>
      <p class="view-sub">
        路径保存为用户环境变量（Windows 用 setx，其他平台写入用户配置），重启软件后自动生效
      </p>
    </header>

    <div
      v-if="!env.llamaValid"
      class="banner"
      :class="scanning ? 'banner-info' : 'banner-warn'"
    >
      <span class="banner-icon">{{ scanning ? '⏳' : '◌' }}</span>
      <div class="banner-body">
        <div class="banner-title">
          {{ scanning ? '正在自动检测 llama.cpp …' : '未检测到 llama.cpp 目录' }}
        </div>
        <div v-if="!scanning">
          {{ scanMsg || '请手动扫描，或在上方设置中配置 LLAMA_CPP_PATH 环境变量。' }}
        </div>
        <div v-else class="banner-actions">
          <button class="btn btn-secondary btn-sm" @click="cancelScan">取消扫描</button>
        </div>
      </div>
      <div v-if="!scanning" class="banner-actions">
        <button class="btn btn-secondary btn-sm" @click="startScan">自动扫描</button>
      </div>
    </div>

    <section class="panel">
      <div class="panel-header">
        <span class="panel-icon">⚙</span>
        <div>
          <h3 class="panel-title">环境变量</h3>
          <p class="panel-sub">llama.cpp 与 models 目录路径</p>
        </div>
      </div>

      <div class="env-grid">
        <div class="input-group">
          <label class="input-label">
            llama.cpp 目录
            <span class="hint">LLAMA_CPP_PATH</span>
          </label>
          <div class="input-row">
            <input
              type="text"
              v-model="env.llamaCppPath"
              placeholder="包含 llama-server.exe 的目录 …"
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
          <label class="input-label">
            models 目录
            <span class="hint">LLAMA_CPP_MODELS_PATH</span>
          </label>
          <div class="input-row">
            <input
              type="text"
              v-model="env.modelsPath"
              placeholder="包含 .gguf 模型的目录 …"
              class="input-field"
              @keyup.enter="saveModelsPath"
            />
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

    <AdvancedParams />

    <ApiPanel />
  </div>
</template>
