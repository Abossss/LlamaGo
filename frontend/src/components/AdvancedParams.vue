<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { useAdvanced, persistAdvanced, generateApiKey } from '../composables/useAdvanced';
import { modelPath } from '../composables/useEnvModels';
import { isValidWebPort, webPort, webPortRangeMessage } from '../composables/useConfig';
import { useServer } from '../composables/useServer';
import { useToast } from '../composables/useToast';
import { hasTauri } from '../lib/hasTauri';
import { MAX_WEB_PORT, MIN_WEB_PORT } from '../lib/const';

const { advanced, aliases } = useAdvanced();
const modelPathRef = modelPath;
const { showToast } = useToast();
const { livePort, configuredWebPort, portChangePending } = useServer();

const expanded = ref(false);
const showKey = ref(false);

// Number inputs yield `''` when cleared; normalize to `null` (= default port).
const webPortInput = computed<number | string>({
  get: () => webPort.value ?? '',
  set: (v) => {
    if (v === '' || v == null) {
      webPort.value = null;
      return;
    }
    const n = typeof v === 'number' ? v : Number(v);
    webPort.value = Number.isFinite(n) ? n : null;
  },
});

const webPortError = computed(() =>
  isValidWebPort(webPort.value) ? '' : webPortRangeMessage(),
);

// Alias for the currently selected model.
const currentAlias = computed({
  get() {
    return aliases.value[modelPathRef.value]?.trim() || '';
  },
  set(v: string) {
    if (!modelPathRef.value) return;
    if (v.trim()) aliases.value[modelPathRef.value] = v.trim();
    else delete aliases.value[modelPathRef.value];
  },
});

// Default to expanded when a model is chosen, collapsed otherwise.
watch(modelPathRef, (m) => {
  expanded.value = !!m;
});

// Persist on blur of any field (debounced via a flag — keep it simple: on change).
watch(
  [
    () => advanced.value.ctx_size,
    () => advanced.value.ngl,
    () => advanced.value.temperature,
    () => advanced.value.max_tokens,
    () => advanced.value.api_key,
    () => webPort.value,
    currentAlias,
  ],
  () => {
    // Never persist an invalid port; the user gets the inline error instead.
    if (hasTauri() && !webPortError.value) persistAdvanced();
  },
  { deep: false },
);

function onGenerateKey() {
  advanced.value.api_key = generateApiKey();
  showKey.value = true;
}

function copyKey() {
  if (!advanced.value.api_key) return;
  navigator.clipboard.writeText(advanced.value.api_key).then(
    () => showToast('API Key 已复制', 'success'),
    () => showToast('复制失败', 'error'),
  );
}
</script>

<template>
  <section class="panel advanced-params">
    <button
      type="button"
      class="advanced-toggle"
      :class="{ 'is-open': expanded }"
      @click="expanded = !expanded"
    >
      <span class="advanced-toggle-icon" aria-hidden="true">⚙</span>
      <span class="advanced-toggle-label">高级参数</span>
      <span class="advanced-toggle-hint">
        {{ expanded ? '收起' : '展开' }}
      </span>
    </button>

    <div v-if="expanded" class="advanced-body">
      <div class="input-group">
        <label class="input-label">
          Web 端口
          <span class="hint">--port（WebUI 地址，留空则自动使用 llama-server 默认端口；重启服务器后生效）</span>
        </label>
        <input
          type="number"
          v-model.number="webPortInput"
          class="input-field"
          :class="{ 'input-invalid': !!webPortError }"
          :min="MIN_WEB_PORT"
          :max="MAX_WEB_PORT"
          step="1"
          placeholder="如 8080"
        />
        <p v-if="webPortError" class="field-error">{{ webPortError }}</p>
        <p v-else-if="portChangePending" class="field-warn">
          已保存端口 {{ configuredWebPort }}，当前运行端口 {{ livePort }}，重启服务器后生效。
        </p>
      </div>

      <div class="input-group">
        <label class="input-label">
          模型别名
          <span class="hint">--alias（WebUI 中显示的模型名）</span>
        </label>
        <input
          type="text"
          v-model="currentAlias"
          class="input-field"
          placeholder="留空则使用文件名（不含 .gguf）"
          :disabled="!modelPath"
        />
      </div>

      <div class="input-group">
        <label class="input-label">
          API Key
          <span class="hint">--api-key（OpenAI 兼容 Bearer Token）</span>
        </label>
        <div class="input-row">
          <input
            :type="showKey ? 'text' : 'password'"
            v-model="advanced.api_key"
            class="input-field"
            placeholder="留空则无需认证"
          />
          <button class="btn btn-secondary btn-sm" @click="onGenerateKey">生成</button>
          <button
            class="btn btn-secondary btn-sm"
            :disabled="!advanced.api_key"
            @click="showKey = !showKey"
          >
            {{ showKey ? '隐藏' : '显示' }}
          </button>
          <button
            class="btn btn-secondary btn-sm"
            :disabled="!advanced.api_key"
            @click="copyKey"
          >
            复制
          </button>
        </div>
      </div>

      <div class="input-group">
        <label class="input-label">
          上下文长度
          <span class="hint">-c（默认：llama-server 默认）</span>
        </label>
        <input
          type="number"
          v-model.number="advanced.ctx_size"
          class="input-field"
          min="512"
          step="512"
          placeholder="如 8192"
        />
      </div>

      <div class="input-group">
        <label class="input-label">
          GPU 层数
          <span class="hint">-ngl（0 = 纯 CPU）</span>
        </label>
        <input
          type="number"
          v-model.number="advanced.ngl"
          class="input-field"
          min="0"
          max="99"
          placeholder="如 33"
        />
      </div>

      <div class="input-group">
        <label class="input-label">
          温度
          <span class="hint">0.0 – 2.0（通过 OpenAI 客户端设置）</span>
        </label>
        <input
          type="number"
          v-model.number="advanced.temperature"
          class="input-field"
          min="0"
          max="2"
          step="0.05"
          placeholder="如 0.7"
        />
      </div>

      <div class="input-group" style="margin-bottom: 0">
        <label class="input-label">
          最大输出 token
          <span class="hint">通过 OpenAI 客户端设置</span>
        </label>
        <input
          type="number"
          v-model.number="advanced.max_tokens"
          class="input-field"
          min="1"
          placeholder="如 1024"
        />
      </div>

      <p class="advanced-hint">
        如启动失败请检查 llama-server 版本是否支持 <code>--alias</code> /
        <code>--api-key</code>（llama.cpp ≥ b2100）。
      </p>
    </div>
  </section>
</template>

<style scoped>
.advanced-params {
  padding: 0;
}

.advanced-toggle {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 16px 22px;
  background: transparent;
  border: none;
  cursor: pointer;
  font-family: var(--font-sans);
  font-size: 14px;
  font-weight: 500;
  color: var(--ink-soft);
  text-align: left;
}

.advanced-toggle:hover {
  background: var(--surface);
}

.advanced-toggle-icon {
  font-size: 16px;
  color: var(--brand);
}

.advanced-toggle-label {
  flex: 1;
}

.advanced-toggle-hint {
  font-size: 11px;
  color: var(--ink-faint);
  font-family: var(--font-mono);
}

.advanced-body {
  padding: 16px 22px 22px;
}

.advanced-hint {
  margin: 14px 0 0;
  font-size: 11.5px;
  color: var(--ink-faint);
  line-height: 1.5;
}

.advanced-hint code {
  font-family: var(--font-mono);
  background: var(--border);
  padding: 1px 5px;
  border-radius: 4px;
}

.field-error,
.field-warn {
  margin: 6px 0 0;
  font-size: 11.5px;
  line-height: 1.5;
}

.field-error {
  color: var(--danger, #c0392b);
}

.field-warn {
  color: var(--warn, #b7791f);
}

.input-invalid {
  border-color: var(--danger, #c0392b);
}
</style>
