<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue';
import { useServer } from '../composables/useServer';
import { modelPath } from '../composables/useEnvModels';
import { aliases, advanced } from '../composables/useAdvanced';
import { useToast } from '../composables/useToast';
import { fetchWithTimeout, isTimeoutError } from '../lib/http';

const {
  running,
  ready,
  webPort,
  serverModelId,
  activeAlias,
  activeKey,
  refreshServerModelInfo,
} = useServer();
const { showToast } = useToast();

const shownModel = computed(() => {
  // While running, only report what the process actually serves; the saved
  // selection may diverge and would be misleading here.
  if (running.value) return serverModelId.value || activeAlias.value;
  return aliases.value[modelPath.value]?.trim() || modelPath.value;
});
const shownKey = computed(() => (running.value ? activeKey.value : advanced.value.api_key));
const showKey = ref(false);
const maskedKey = '\u2022'.repeat(8);

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

// The active model id is fetched by the shared server state once the process
// reports ready; the Settings page may mount later, so read it once here too.
onMounted(() => {
  if (running.value) void refreshServerModelInfo();
});

// ── Connectivity test ────────────────────────────────────────────────
const testing = ref(false);
const testResult = ref<{ ok: boolean; text: string } | null>(null);

watch(running, () => {
  testResult.value = null;
});

async function testConnection() {
  if (!running.value) {
    showToast('请先启动服务器', 'warning');
    return;
  }
  testing.value = true;
  testResult.value = null;
  const started = performance.now();
  try {
    const headers: Record<string, string> = {};
    const key = shownKey.value?.trim();
    if (key) headers.Authorization = `Bearer ${key}`;
    const res = await fetchWithTimeout(
      `${baseUrl.value}/v1/models`,
      { headers },
      4000,
    );
    const ms = Math.max(1, Math.round(performance.now() - started));
    if (res.ok) {
      testResult.value = { ok: true, text: `连接正常 · ${ms} ms` };
    } else if (res.status === 401) {
      testResult.value = { ok: false, text: '认证失败（401），请检查 API Key' };
    } else {
      testResult.value = {
        ok: false,
        text: `服务器返回 ${res.status}${res.status === 503 ? '（模型仍在加载）' : ''}`,
      };
    }
  } catch (err) {
    testResult.value = {
      ok: false,
      text: isTimeoutError(err)
        ? '连接超时（4 秒），请确认端口与进程状态'
        : ready.value
          ? '无法连接服务器'
          : '模型仍在加载，请稍候',
    };
  } finally {
    testing.value = false;
  }
}

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
        <p class="panel-sub">OpenAI 兼容接口，服务器就绪后自动获取模型名称与调用地址</p>
      </div>
    </div>

    <div class="api-row">
      <span class="api-label">Base URL</span>
      <code class="api-value">{{ baseUrl }}</code>
    </div>

    <div class="api-row">
      <span class="api-label">模型名称</span>
      <code class="api-value" v-if="shownModel">{{ shownModel }}</code>
      <span class="api-value api-missing" v-else>
        {{ running ? (ready ? '获取中…' : '模型加载中…') : '未选择模型' }}
      </span>
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
      <code class="api-value" v-if="shownKey">{{ showKey ? shownKey : maskedKey }}</code>
      <span class="api-value api-missing" v-else>未设置，无需认证</span>
      <template v-if="shownKey">
        <button class="btn btn-ghost btn-sm" @click="showKey = !showKey">
          {{ showKey ? '隐藏' : '显示' }}
        </button>
        <button class="btn btn-ghost btn-sm" @click="copyKey">复制</button>
      </template>
    </div>

    <div class="api-endpoints">
      <span class="api-label">端点</span>
      <ul class="api-endpoint-list">
        <li>/v1/chat/completions</li>
        <li>/v1/completions</li>
        <li>/v1/models</li>
        <li>/v1/embeddings <span class="api-endpoint-note">需模型支持</span></li>
      </ul>
    </div>

    <div class="api-test">
      <button
        class="btn btn-secondary btn-sm"
        :disabled="!running || testing"
        @click="testConnection"
      >
        {{ testing ? '测试中…' : '测试连接' }}
      </button>
      <span
        v-if="testResult"
        class="api-test-result"
        :class="testResult.ok ? 'ok' : 'bad'"
      >
        {{ testResult.text }}
      </span>
      <span v-else class="api-test-hint">
        {{
          running && !ready
            ? '模型加载完成后可测试'
            : '请求 /v1/models 验证连通性'
        }}
      </span>
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

.api-endpoint-note {
  font-family: var(--font-sans);
  font-size: 10px;
  color: var(--ink-faint);
  margin-left: 2px;
}

.api-test {
  display: flex;
  align-items: center;
  gap: 10px;
  margin: 12px 0 14px;
  flex-wrap: wrap;
}

.api-test-result {
  font-size: 12px;
}

.api-test-result.ok {
  color: var(--success);
}

.api-test-result.bad {
  color: var(--error);
}

.api-test-hint {
  font-size: 12px;
  color: var(--ink-faint);
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
