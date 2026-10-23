<script setup lang="ts">
import { ref } from 'vue';
import { useServer } from '../composables/useServer';
import { useToast } from '../composables/useToast';
import { cmd } from '../lib/tauri';
import { hasTauri } from '../lib/hasTauri';

const { logs, clearLog, logFollow, setLogFollow } = useServer();
const { showToast } = useToast();

const terminalRef = ref<HTMLElement | null>(null);

function logClass(line: string) {
  if (line.startsWith('❌')) return 'log-error';
  if (line.startsWith('✅')) return 'log-success';
  if (line.startsWith('⏹')) return 'log-stop';
  return 'log-line';
}

// Scrolling up pauses auto-follow; returning to the bottom resumes it.
function onScroll() {
  const el = terminalRef.value;
  if (!el) return;
  const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
  if (atBottom !== logFollow.value) setLogFollow(atBottom);
}

function stamp(): string {
  return new Date()
    .toISOString()
    .replace(/[:.]/g, '-')
    .slice(0, 19);
}

async function exportLogs() {
  const text = logs.value.map((l) => l.text).join('\n');
  const name = `llamago-${stamp()}.log`;
  if (!hasTauri()) {
    downloadInBrowser(text, name);
    return;
  }
  try {
    const path = await cmd.export_log(text, name);
    if (path) showToast(`日志已导出到 ${path}`, 'success', 6000);
  } catch (err) {
    showToast(`导出失败: ${err}`, 'error');
  }
}

function downloadInBrowser(text: string, name: string) {
  const url = URL.createObjectURL(
    new Blob([text], { type: 'text/plain;charset=utf-8' }),
  );
  const a = document.createElement('a');
  a.href = url;
  a.download = name;
  a.click();
  URL.revokeObjectURL(url);
}
</script>

<template>
  <section class="log-panel">
    <div class="log-header">
      <span class="log-header-label">
        运行日志
        <span v-if="logs.length" class="log-count">{{ logs.length }}</span>
      </span>
      <div class="log-actions">
        <button
          class="btn btn-ghost btn-sm log-follow-btn"
          :class="{ active: logFollow }"
          :title="logFollow ? '新日志自动滚动到底部' : '已暂停滚动，点击恢复'"
          @click="setLogFollow(!logFollow)"
        >
          ⇣ 跟随{{ logFollow ? '' : '（已暂停）' }}
        </button>
        <button class="btn btn-ghost btn-sm" :disabled="!logs.length" @click="exportLogs">
          导出
        </button>
        <button v-if="logs.length" class="btn btn-ghost btn-sm" @click="clearLog">
          清空
        </button>
      </div>
    </div>
    <div ref="terminalRef" class="terminal-output" @scroll.passive="onScroll">
      <div v-for="entry in logs" :key="entry.id" :class="logClass(entry.text)">
        {{ entry.text }}
      </div>
      <div v-if="logs.length === 0" class="log-empty">等待日志输出…</div>
    </div>
  </section>
</template>

<style scoped>
.log-panel {
  display: flex;
  flex-direction: column;
  min-height: 0;
}

.log-count {
  font-family: var(--font-mono);
  font-size: 10.5px;
  color: var(--ink-faint);
  margin-left: 6px;
}

.log-actions {
  display: flex;
  align-items: center;
  gap: 2px;
}

.log-follow-btn.active {
  color: var(--success);
}
</style>
