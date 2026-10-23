<script setup lang="ts">
import { watch } from 'vue';
import { useServer } from '../composables/useServer';
import { useEnvModels, env } from '../composables/useEnvModels';
import { useConfig } from '../composables/useConfig';
import LogPanel from './LogPanel.vue';

const emit = defineEmits<{ goto: [tab: 'launch' | 'settings'] }>();

const { execPath, modelPath, models } = useEnvModels();
const {
  running,
  startServer,
  stopServer,
  openWebUI,
  livePort,
  configuredWebPort,
  portChangePending,
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

    <section class="panel">
      <div class="panel-header">
        <span class="panel-icon">▶</span>
        <div>
          <h3 class="panel-title">服务器</h3>
          <p class="panel-sub">可执行文件由 LLAMA_CPP_PATH 环境变量决定</p>
        </div>
      </div>

      <div class="input-group">
        <label class="input-label">
          可执行文件
          <span class="hint">llama-server.exe</span>
        </label>
        <input type="text" :value="execPath" readonly class="input-field" placeholder="来自 LLAMA_CPP_PATH" />
      </div>

      <div class="input-group">
        <label class="input-label">
          模型
          <span class="hint">.gguf</span>
        </label>
        <select v-model="modelPath" class="input-field" :disabled="models.length === 0">
          <option value="" disabled>— 请选择模型 —</option>
          <option v-for="m in models" :key="m" :value="m">{{ m.split(/[\\/]/).pop() }}</option>
        </select>
        <div v-if="models.length === 0" class="model-count">
          {{
            env.modelsValid
              ? 'models 目录中没有 .gguf 模型'
              : '未配置 models 目录（LLAMA_CPP_MODELS_PATH）'
          }}
        </div>
      </div>

      <div class="control-bar">
        <button
          class="btn btn-primary"
          :disabled="running || !execPath || !modelPath"
          @click="startServer"
        >
          <span v-if="running">⏳</span>
          <span v-else>▶</span>
          {{ running ? '启动中…' : '启动服务器' }}
        </button>
        <button class="btn btn-danger" :disabled="!running" @click="stopServer">⏹ 停止</button>
        <button v-if="running" class="btn btn-secondary" @click="openWebUI">🌐 WebUI</button>
      </div>

      <div v-if="portChangePending" class="banner banner-warn" style="margin-top: 14px">
        <span class="banner-icon">↻</span>
        <div class="banner-body">
          <div class="banner-title">Web 端口已更改，需重启服务器</div>
          <div>
            当前运行在 {{ livePort }}，已保存端口 {{ configuredWebPort }}，重启服务器后生效。
          </div>
        </div>
      </div>
    </section>

    <LogPanel />
  </div>
</template>
