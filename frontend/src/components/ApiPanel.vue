<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue';
import { useServer } from '../composables/useServer';
import { modelPath } from '../composables/useEnvModels';
import { aliases, advanced } from '../composables/useAdvanced';
import { useToast } from '../composables/useToast';

const { running, webPort, livePort, fetchStatusFull } = useServer();
const { showToast } = useToast();

// Live "active" run metadata (from the backend, reflects the running process).
const activeAlias = ref('');
const activeKey = ref<string | null>(null);
// Exact model id served by the running process (alias, or the model path when
// no alias is set). Fetched from `/v1/models` once the server is up.
const serverModelId = ref('');
let modelReqToken = 0;

const shownModel = computed(
  () =>
    serverModelId.value ||
    activeAlias.value ||
    aliases.value[modelPath.value]?.trim() ||
    modelPath.value,
);
const shownKey = computed(() => activeKey.value ?? advanced.value.api_key);

const baseUrl = computed(() => `http://127.0.0.1:${webPort.value}`);
const endpoint = computed(() => `${baseUrl.value}/v1/chat/completions`);

function buildCurl(): string {
  const model = shownModel.value || 'model';
  const body = JSON.stringify({
    model,
    messages: [{ role: 'user', content: 'hi' }],
  });
  let s = `curl ${endpoint.value} \\\n  -H "Content-Type: application/json"`;
  if (shownKey.value) {
    s += ` \\\n  -H "Authorization: Bearer ${shownKey.value}"`;
  }
  s += ` \\\n  -d '${body}'`;
  return s;
}

const curl = computed(buildCurl);

// Ask the running server for the model id it actually serves. Retries while
// the process is still loading the model.
async function fetchServerModel() {
  if (!running.value) return;
  const token = ++modelReqToken;
  for (let attempt = 0; attempt < 30 && running.value; attempt++) {
    try {
      const headers: Record<string, string> = {};
      if (activeKey.value) headers.Authorization = `Bearer ${activeKey.value}`;
      const res = await fetch(`http://127.0.0.1:${livePort.value}/v1/models`, {
        headers,
      });
      if (res.ok) {
        const body = await res.json();
        const id = body?.data?.[0]?.id;
        if (token === modelReqToken && typeof id === 'string' && id) {
          serverModelId.value = id;
        }
        return;
      }
    } catch {
      /* server not listening yet */
    }
    await new Promise((r) => setTimeout(r, 500));
  }
}

// Poll the backend's full status to keep alias + key in sync with the process.
async function refreshActive() {
  try {
    const s = await fetchStatusFull();
    activeAlias.value = s.model_alias || '';
    activeKey.value = s.api_key || null;
  } catch {
    /* ignore */
  }
  await fetchServerModel();
}

// Re-poll on start/stop transitions.
watch(running, (v) => {
  if (v) refreshActive();
  else {
    modelReqToken++;
    activeAlias.value = '';
    activeKey.value = null;
    serverModelId.value = '';
  }
});

// The Settings page can mount while the server is already running, in which
// case no start transition fires; read the live model info once here.
onMounted(() => {
  if (running.value) refreshActive();
});

onUnmounted(() => {
  modelReqToken++;
});

function copyText(text: string, label: string) {
  navigator.clipboard.writeText(text).then(
    () => showToast(`${label} 已复制`, 'success'),
    () => showToast('复制失败', 'error'),
  );
}

function copyCurl() {
  copyText(curl.value, 'curl 示例');
}

function copyKey() {
  if (shownKey.value) copyText(shownKey.value, 'API Key');
}
</script>

<template>
  <section class="panel api-panel">
    <div class="panel-header">
      <span class="panel-icon">🔌</span>
      <div>
        <h3 class="panel-title">API 调用</h3>
        <p class="panel-sub">OpenAI 兼容接口，服务器启动后自动获取模型名称与调用地址</p>
      </div>
    </div>

    <div class="api-row">
      <span class="api-label">Base URL</span>
      <code class="api-value">{{ baseUrl }}</code>
    </div>

    <div class="api-row">
      <span class="api-label">模型名称</span>
      <code class="api-value" v-if="shownModel">{{ shownModel }}</code>
      <span class="api-value api-missing" v-else>启动后自动获取</span>
      <button
        v-if="shownModel"
        class="btn btn-ghost btn-sm"
        @click="copyText(shownModel, '模型名称')"
      >
        复制
      </button>
    </div>

    <div class="api-row">
      <span class="api-label">调用 API</span>
      <code class="api-value">{{ endpoint }}</code>
      <button class="btn btn-ghost btn-sm" @click="copyText(endpoint, '调用 API')">
        复制
      </button>
    </div>

    <div class="api-row">
      <span class="api-label">API Key</span>
      <code class="api-value" v-if="shownKey">{{ shownKey }}</code>
      <span class="api-value api-missing" v-else>未设置，无需认证</span>
      <button
        v-if="shownKey"
        class="btn btn-ghost btn-sm"
        @click="copyKey"
      >
        复制
      </button>
    </div>

    <div class="api-endpoints">
      <span class="api-label">端点</span>
      <ul class="api-endpoint-list">
        <li>/v1/chat/completions</li>
        <li>/v1/completions</li>
        <li>/v1/models</li>
        <li>/v1/embeddings</li>
      </ul>
    </div>

    <div class="input-group" style="margin-bottom: 0">
      <label class="input-label">
        curl 示例
        <span class="hint">点击复制</span>
      </label>
      <pre class="api-curl" @click="copyCurl" title="点击复制">{{ curl }}</pre>
    </div>
  </section>
</template>

<style scoped>
.api-panel {
  margin-bottom: 16px;
}

.api-row,
.api-endpoints {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 10px;
  flex-wrap: wrap;
}

.api-label {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--ink-faint);
  min-width: 64px;
}

.api-value {
  font-family: var(--font-mono);
  font-size: 12.5px;
  color: var(--ink-soft);
  background: var(--border);
  padding: 3px 8px;
  border-radius: 6px;
  word-break: break-all;
  flex: 1;
}

.api-value.api-missing {
  color: var(--ink-faint);
  background: transparent;
}

.api-endpoint-list {
  list-style: none;
  padding: 0;
  margin: 0;
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--ink-soft);
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.api-endpoint-list li {
  background: var(--border);
  padding: 2px 8px;
  border-radius: 5px;
}

.api-curl {
  margin: 0;
  background: var(--surface-dark);
  color: var(--ink-on-dark);
  font-family: var(--font-mono);
  font-size: 11.5px;
  line-height: 1.6;
  padding: 12px 14px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  white-space: pre-wrap;
  word-break: break-all;
  transition: opacity 0.2s ease;
}

.api-curl:hover {
  opacity: 0.85;
}
</style>
