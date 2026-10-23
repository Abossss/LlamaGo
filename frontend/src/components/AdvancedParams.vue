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
const { running, livePort, configuredWebPort, portChangePending } = useServer();

const expanded = ref(!!modelPath.value);
const showKey = ref(false);

// Number inputs yield `''` when cleared; normalize to `null` (= server default)
// so a cleared field is omitted from the arguments instead of passed as `-c ''`.
function toOptionalNumber(v: number | string | null | undefined): number | null {
  if (v === '' || v == null) return null;
  const n = typeof v === 'number' ? v : Number(v);
  return Number.isFinite(n) ? n : null;
}

const webPortInput = computed<number | string>({
  get: () => webPort.value ?? '',
  set: (v) => {
    webPort.value = toOptionalNumber(v);
  },
});

const ctxSizeInput = computed<number | string>({
  get: () => advanced.value.ctx_size ?? '',
  set: (v) => {
    advanced.value.ctx_size = toOptionalNumber(v);
  },
});

const nglInput = computed<number | string>({
  get: () => advanced.value.ngl ?? '',
  set: (v) => {
    advanced.value.ngl = toOptionalNumber(v);
  },
});

const webPortError = computed(() =>
  isValidWebPort(webPort.value) ? '' : webPortRangeMessage(),
);

const ctxSizeError = computed(() => {
  const v = advanced.value.ctx_size;
  if (v == null) return '';
  return Number.isInteger(v) && v >= 1 ? '' : '上下文长度需为不小于 1 的整数';
});

const nglError = computed(() => {
  const v = advanced.value.ngl;
  if (v == null) return '';
  return Number.isInteger(v) && v >= 0 ? '' : 'GPU 层数需为不小于 0 的整数';
});

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

// Persist changes after a short pause so typing in a field does not rewrite
// config.json on every keystroke.
watch(
  [
    () => advanced.value.ctx_size,
    () => advanced.value.ngl,
    () => advanced.value.extra_args,
    () => advanced.value.api_key,
    () => webPort.value,
    currentAlias,
  ],
  () => {
    // Never persist invalid numbers; the user gets the inline error instead.
    if (hasTauri() && !webPortError.value && !ctxSizeError.value && !nglError.value) {
      persistAdvanced();
    }
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
      <p class="advanced-scope">
        以下参数为全局配置，对所有模型生效。启动时按「模型 → 上下文 → GPU 层数 → 别名 → 其他参数 → 端口」的顺序拼接。
      </p>
      <p v-if="running" class="advanced-restart-note">
        服务器正在运行：修改端口、别名、API Key、上下文长度、GPU 层数或其他参数后，需重启服务器才会生效。
      </p>

      <div class="input-group">
        <label class="input-label" for="adv-web-port">
          Web 端口
          <span class="hint">--port（WebUI 地址，留空则自动使用 llama-server 默认端口）</span>
        </label>
        <input
          id="adv-web-port"
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
          当前运行端口 {{ livePort }}，下次启动将使用 {{ configuredWebPort }}，重启服务器后生效。
        </p>
      </div>

      <div class="input-group">
        <label class="input-label" for="adv-alias">
          模型别名
          <span class="hint">--alias（WebUI 中显示的模型名）</span>
        </label>
        <input
          id="adv-alias"
          type="text"
          v-model="currentAlias"
          class="input-field"
          placeholder="留空则使用文件名（不含 .gguf）"
          :disabled="!modelPath"
        />
      </div>

      <div class="input-group">
        <label class="input-label" for="adv-api-key">
          API Key
          <span class="hint">--api-key（OpenAI 兼容 Bearer Token）</span>
        </label>
        <div class="input-row">
          <input
            id="adv-api-key"
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
        <label class="input-label" for="adv-ctx">
          上下文长度
          <span class="hint">-c（默认：llama-server 默认）</span>
        </label>
        <input
          id="adv-ctx"
          type="number"
          v-model.number="ctxSizeInput"
          class="input-field"
          :class="{ 'input-invalid': !!ctxSizeError }"
          min="512"
          step="512"
          placeholder="如 8192"
        />
        <p v-if="ctxSizeError" class="field-error">{{ ctxSizeError }}</p>
      </div>

      <div class="input-group">
        <label class="input-label" for="adv-ngl">
          GPU 层数
          <span class="hint">-ngl（0 = 纯 CPU）</span>
        </label>
        <input
          id="adv-ngl"
          type="number"
          v-model.number="nglInput"
          class="input-field"
          :class="{ 'input-invalid': !!nglError }"
          min="0"
          max="99"
          placeholder="如 33"
        />
        <p v-if="nglError" class="field-error">{{ nglError }}</p>
      </div>

      <div class="input-group" style="margin-bottom: 0">
        <label class="input-label" for="adv-extra-args">
          其他启动参数
          <span class="hint">原样追加，支持引号</span>
        </label>
        <textarea
          id="adv-extra-args"
          v-model="advanced.extra_args"
          class="input-field"
          rows="2"
          spellcheck="false"
          placeholder="如 --mlock --threads 8 --flash-attn"
        ></textarea>
        <p class="field-hint">
          温度、最大输出 token 等生成参数由 OpenAI 客户端在请求中设置，不在此处配置。
        </p>
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

.advanced-scope {
  margin: 0 0 12px;
  font-size: 12px;
  line-height: 1.5;
  color: var(--ink-faint);
}

.advanced-restart-note {
  margin: 0 0 14px;
  padding: 8px 12px;
  font-size: 12px;
  line-height: 1.5;
  color: var(--warn, #b7791f);
  background: var(--warn-light, rgba(201, 168, 66, 0.12));
  border-radius: var(--radius-sm);
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

.field-hint {
  margin: 6px 0 0;
  font-size: 11.5px;
  line-height: 1.5;
  color: var(--ink-faint);
}

textarea.input-field {
  resize: vertical;
  min-height: 54px;
  font-family: var(--font-mono);
  font-size: 12.5px;
  line-height: 1.6;
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
