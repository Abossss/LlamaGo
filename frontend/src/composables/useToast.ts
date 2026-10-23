import { ref } from 'vue';
import { TOAST_ICONS, MAX_TOASTS, type ToastType } from '../lib/const';

export interface ToastItem {
  id: number;
  type: ToastType;
  message: string;
  icon: string;
  duration: number;
}

/**
 * App-level toast state. Mirrors the original `toasts`/`showToast`/`dismissToast`
 * in index.html. Exposed as a module singleton so any composable/component can
 * push a toast.
 */
const toasts = ref<ToastItem[]>([]);
let toastId = 0;

function showToast(message: string, type: ToastType = 'info', duration = 3000) {
  const id = ++toastId;
  toasts.value.push({
    id,
    type,
    message,
    icon: TOAST_ICONS[type] || 'ℹ',
    duration,
  });
  setTimeout(() => dismissToast(id), duration);
  if (toasts.value.length > MAX_TOASTS) {
    toasts.value.shift();
  }
}

function dismissToast(id: number) {
  const idx = toasts.value.findIndex((t) => t.id === id);
  if (idx > -1) toasts.value.splice(idx, 1);
}

export function useToast() {
  return {
    toasts,
    showToast,
    dismissToast,
  };
}
