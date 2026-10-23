<script setup lang="ts">
import { useServer } from '../composables/useServer';

defineProps<{
  collapsed: boolean;
  tab: 'launch' | 'settings';
}>();

const emit = defineEmits<{
  select: [tab: 'launch' | 'settings'];
  toggleCollapse: [];
}>();

const { running, pid, webPort } = useServer();
</script>

<template>
  <aside class="sidebar" :class="{ collapsed }">
    <div class="sidebar-brand">
      <span class="brand-mark">L<em>g</em></span>
      <h1 class="brand-title">Llama<span>Go</span></h1>
      <p class="brand-tagline">llama.cpp GUI 启动器</p>
    </div>

    <p class="nav-section-label">导航</p>
    <nav class="sidebar-nav">
      <button
        class="nav-item"
        :class="{ active: tab === 'launch' }"
        @click="emit('select', 'launch')"
        title="启动"
      >
        <svg
          viewBox="0 0 24 24"
          width="18"
          height="18"
          fill="none"
          stroke="currentColor"
          stroke-width="1.8"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <polygon points="6 3 20 12 6 21 6 3" />
        </svg>
        <span>启动</span>
        <span class="nav-kbd">2</span>
      </button>
      <button
        class="nav-item"
        :class="{ active: tab === 'settings' }"
        @click="emit('select', 'settings')"
        title="设置"
      >
        <svg
          viewBox="0 0 24 24"
          width="18"
          height="18"
          fill="none"
          stroke="currentColor"
          stroke-width="1.8"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <circle cx="12" cy="12" r="3" />
          <path
            d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"
          />
        </svg>
        <span>设置</span>
        <span class="nav-kbd">1</span>
      </button>
    </nav>

    <div class="sidebar-footer">
      <div class="server-status">
        <span class="dot" :class="running ? 'dot-running' : 'dot-stopped'" />
        <div class="server-copy">
          <span class="status-label">{{ running ? '运行中' : '已停止' }}</span>
          <span class="status-meta" v-if="running && pid">PID {{ pid }} · :{{ webPort }}</span>
          <span class="status-meta" v-else>服务器未启动</span>
        </div>
      </div>
      <button
        class="collapse-btn"
        @click="emit('toggleCollapse')"
        :title="collapsed ? '展开侧边栏' : '收起侧边栏'"
      >
        <svg
          v-if="!collapsed"
          viewBox="0 0 24 24"
          width="17"
          height="17"
          fill="none"
          stroke="currentColor"
          stroke-width="1.8"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <polyline points="15 18 9 12 15 6" />
        </svg>
        <svg
          v-else
          viewBox="0 0 24 24"
          width="17"
          height="17"
          fill="none"
          stroke="currentColor"
          stroke-width="1.8"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <polyline points="9 18 15 12 9 6" />
        </svg>
        <span>{{ collapsed ? '展开' : '收起侧边栏' }}</span>
      </button>
    </div>
  </aside>
</template>
