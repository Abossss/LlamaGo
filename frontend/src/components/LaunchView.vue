<script setup lang="ts">
import { watch } from 'vue';
import { useServer } from '../composables/useServer';
import { useEnvModels, env } from '../composables/useEnvModels';
import { useConfig } from '../composables/useConfig';
import LogPanel from './LogPanel.vue';

const emit = defineEmits<{ goto: [tab: 'launch' | 'settings'] }>();

const { execPath, modelPath, models, scanning, refreshModels } = useEnvModels();
const {
  running,
  starting,
  ready,
  startServer,
  stopServer,
  restartServer,
  openWebUI,
  livePort,
  configuredWebPort,
  portChangePending,
  launchCommand,
  lastExit,
} = useServer();
const { saveSelection } = useConfig();

watch(modelPath, saveSelection);
</script>

<template>
  <div class="view">
    <header class="view-header">
      <p class="view-kicker">运行</p>
      <h2 class="view-title">启动</h2>
      <p class="view-sub">选择 .gguf 模型并运行 llama-server，实时查看运行日志</p>
    </header>

    <div v-if="!env.llamaValid" class="banner banner-warn">
      <span class="banner-icon">◌</span>
      <div class="banner-body">
        <div class="banner-title">未配置 llama.cpp 目录</div>
        <div>请先在设置中配置 LLAMA_CPP_PATH 环境变量。</div>
        <div class="banner-actions">
          <button class="btn btn-secondary btn-sm" @click="emit('goto', 'settings')">
            前往设置 →
          </button>
        </div>
      </div>
    </div>

    <div v-if="lastExit" class="banner banner-error">
      <span class="banner-icon">✕</span>
      <div class="banner-body">
        <div class="banner-title">服务器意外退出</div>
        <div>
          退出码：{{ lastExit.code ?? '未知' }}。请查看下方日志，常见原因包括端口占用、模型文件损坏或显存不足。
        </div>
      </div>
    </div>

    <section class="panel">
      <div class="panel-header">
        <span class="panel-icon">▶</span>
        <div>
          <h3 class="panel-title">服务器</h3>
          <p class="panel-sub">可执行文件由 LLAMA_CPP_PATH 环境变量决定</p>
        </div>
      </div>

      <div class="input-group">
        <label class="input-label" for="exec-path">
          可执行文件
          <span class="hint">llama-server</span>
        </label>
        <input
          id="exec-path"
          type="text"
          :value="execPath"
          readonly
          class="input-field"
          placeholder="来自 LLAMA_CPP_PATH"
        />
      </div>

      <div class="input-group">
        <div class="input-label-row">
          <label class="input-label" for="model-select">
            模型
            <span class="hint">.gguf</span>
          </label>
          <button
            class="btn btn-ghost btn-sm"
            :disabled="scanning || running"
            title="重新扫描 models 目录"
            @click="refreshModels"
          >
            ↻ 刷新
          </button>
        </div>
        <select
          id="model-select"
          v-model="modelPath"
          class="input-field"
          :disabled="models.length === 0 || running || starting"
        >
          <option value="" disabled>— 请选择模型 —</option>
          <option v-for="m in models" :key="m" :value="m">{{ m.split(/[\\/]/).pop() }}</option>
        </select>
        <div v-if="running" class="model-count">服务器运行中，停止后可切换模型</div>
        <div v-else-if="models.length === 0" class="model-count">
          <template v-if="env.modelsValid">
            models 目录中没有可用的 .gguf 模型
            <button class="btn btn-ghost btn-sm" :disabled="scanning" @click="refreshModels">
              重新扫描
            </button>
          </template>
          <template v-else>
            未配置 models 目录（LLAMA_CPP_MODELS_PATH）
            <button class="btn btn-ghost btn-sm" @click="emit('goto', 'settings')">
              前往设置
            </button>
          </template>
        </div>
      </div>

      <div class="control-bar">
        <button
          class="btn btn-primary"
          :disabled="starting || running || !execPath || !modelPath"
          @click="startServer"
        >
          <span v-if="starting">⏳</span>
          <span v-else-if="running && !ready">◌</span>
          <span v-else-if="running">✓</span>
          <span v-else>▶</span>
          {{
            starting
              ? '启动中…'
              : running && !ready
                ? '模型加载中…'
                : running
                  ? '运行中'
                  : '启动服务器'
          }}
        </button>
        <button class="btn btn-danger" :disabled="!running || starting" @click="stopServer()">
          ⏹ 停止
        </button>
        <button class="btn btn-secondary" :disabled="!running || starting" @click="restartServer">
          ↻ 重启
        </button>
        <button
          class="btn btn-secondary"
          :disabled="!ready"
          :title="running && !ready ? '模型加载完成后可用' : undefined"
          @click="openWebUI"
        >
          🌐 WebUI
        </button>
      </div>

      <div v-if="portChangePending" class="banner banner-warn" style="margin: 14px 0 0">
        <span class="banner-icon">↻</span>
        <div class="banner-body">
          <div class="banner-title">Web 端口已更改，需重启服务器</div>
          <div>
            当前运行在 {{ livePort }}，下次启动将使用 {{ configuredWebPort }}。
          </div>
          <div class="banner-actions">
            <button class="btn btn-secondary btn-sm" @click="restartServer">立即重启</button>
          </div>
        </div>
      </div>

      <details v-if="modelPath" class="launch-preview">
        <summary>启动命令预览</summary>
        <pre class="launch-command">{{ launchCommand }}</pre>
        <p class="launch-preview-hint">
          API Key 通过 <code>LLAMA_API_KEY</code> 环境变量传入，不会出现在命令行中。
        </p>
      </details>
    </section>

    <LogPanel />
  </div>
</template>

<style scoped>
.input-label-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.input-label-row .input-label {
  flex: 1;
  margin-bottom: 0;
}

.input-label-row + select {
  margin-top: 6px;
}

.launch-preview {
  margin-top: 18px;
  border-top: 1px solid var(--border);
  padding-top: 12px;
}

.launch-preview summary {
  cursor: pointer;
  font-size: 12.5px;
  color: var(--ink-faint);
  user-select: none;
}

.launch-preview summary:hover {
  color: var(--ink-muted);
}

.launch-command {
  margin: 10px 0 0;
  background: var(--surface-dark);
  color: var(--ink-on-dark);
  font-family: var(--font-mono);
  font-size: 11.5px;
  line-height: 1.7;
  padding: 10px 12px;
  border-radius: var(--radius-sm);
  white-space: pre-wrap;
  word-break: break-all;
}

.launch-preview-hint {
  margin: 8px 0 0;
  font-size: 11.5px;
  color: var(--ink-faint);
}

.launch-preview-hint code {
  font-family: var(--font-mono);
  background: var(--border);
  padding: 1px 5px;
  border-radius: 4px;
}
</style>
