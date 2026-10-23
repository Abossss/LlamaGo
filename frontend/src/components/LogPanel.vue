<script setup lang="ts">
import { useServer } from '../composables/useServer';

const { logs, clearLog } = useServer();

function logClass(line: string) {
  if (line.startsWith('❌')) return 'log-error';
  if (line.startsWith('✅')) return 'log-success';
  if (line.startsWith('⏹')) return 'log-stop';
  return 'log-line';
}
</script>

<template>
  <div class="log-header">
    <span class="log-header-label">运行日志</span>
    <button v-if="logs.length" class="btn btn-ghost" @click="clearLog">清空</button>
  </div>
  <div class="terminal-output">
    <div v-for="(line, i) in logs" :key="i" :class="logClass(line)">{{ line }}</div>
    <div v-if="logs.length === 0" class="log-empty">等待日志输出…</div>
  </div>
</template>
