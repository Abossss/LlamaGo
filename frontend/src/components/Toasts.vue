<script setup lang="ts">
import type { ToastItem } from '../composables/useToast';
import { useToast } from '../composables/useToast';

defineProps<{ toasts: ToastItem[] }>();
const { dismissToast } = useToast();
</script>

<template>
  <TransitionGroup name="toast">
    <div
      v-for="t in toasts"
      :key="t.id"
      :class="['toast', 'toast-' + t.type]"
      @click="dismissToast(t.id)"
    >
      <div class="toast-icon">{{ t.icon }}</div>
      <div class="toast-content">{{ t.message }}</div>
      <button class="toast-close" @click.stop="dismissToast(t.id)">×</button>
      <div class="toast-progress">
        <div class="toast-progress-bar" :style="{ animationDuration: t.duration / 1000 + 's' }"></div>
      </div>
    </div>
  </TransitionGroup>
</template>
