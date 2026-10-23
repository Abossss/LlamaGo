<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue';
import Sidebar from './components/Sidebar.vue';
import SettingsView from './components/SettingsView.vue';
import LaunchView from './components/LaunchView.vue';
import Toasts from './components/Toasts.vue';
import { useToast } from './composables/useToast';
import { loadModels, refreshEnv, env } from './composables/useEnvModels';
import { useScan } from './composables/useScan';
import { useConfig } from './composables/useConfig';
import { bindServerEvents } from './composables/useServer';
import { subscribe } from './lib/tauri';
import { hasTauri } from './lib/hasTauri';

// ── Layout: sidebar collapse, persisted in localStorage ──
const collapsed = ref(localStorage.getItem('llamago.collapsed') === '1');
watch(collapsed, (v) => localStorage.setItem('llamago.collapsed', v ? '1' : '0'));

// ── View: 'launch' | 'settings' ──
const tab = ref<'launch' | 'settings'>('launch');

const { toasts } = useToast();
const { startScan } = useScan();
const { restoreLastSelection } = useConfig();

let unlistenToast: (() => void) | null = null;

// View switching: 1 = settings, 2 = launch. Skipped while typing in an input.
function onKey(e: KeyboardEvent) {
  if (e.target && /^(INPUT|TEXTAREA|SELECT)$/.test((e.target as HTMLElement).tagName)) return;
  if (e.key === '1') tab.value = 'settings';
  else if (e.key === '2') tab.value = 'launch';
}

onMounted(async () => {
  window.addEventListener('keydown', onKey);
  if (hasTauri()) {
    unlistenToast = await subscribe<{ message: string; type?: string }>('toast', (e: { message: string; type?: string }) => {
      useToast().showToast(e.message, (e.type as any) || 'info');
    });
    // Bind the backend event stream (log-line / server-status / process-exit)
    // and backfill log history. The root lives for the whole app lifetime, so
    // these are never unlistened.
    await bindServerEvents();
  }
  await refreshEnv();
  if (env.llamaValid) {
    await loadModels();
  } else {
    startScan();
  }
  await restoreLastSelection();

  // One-time hint: closing the window hides to the tray rather than quitting.
  if (hasTauri() && !localStorage.getItem('llamago.trayHintShown')) {
    localStorage.setItem('llamago.trayHintShown', '1');
    setTimeout(() => {
      useToast().showToast(
        '关闭窗口后 LlamaGo 会隐藏到系统托盘，可从托盘菜单重新打开或退出',
        'info',
        7000,
      );
    }, 1500);
  }
});

onUnmounted(() => {
  window.removeEventListener('keydown', onKey);
  unlistenToast?.();
});
</script>

<template>
  <div class="toast-container" role="status" aria-live="polite">
    <Toasts :toasts="toasts" />
  </div>

  <div class="app-shell">
    <Sidebar
      :collapsed="collapsed"
      :tab="tab"
      @select="tab = $event"
      @toggle-collapse="collapsed = !collapsed"
    />

    <main class="main">
      <div class="view-wrap" :class="'view-wrap--' + tab">
        <Transition name="view" mode="out-in">
          <LaunchView v-if="tab === 'launch'" key="launch" />
          <SettingsView v-else key="settings" @goto="tab = $event" />
        </Transition>
      </div>
    </main>
  </div>
</template>
