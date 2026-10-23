import { computed, ref, watch, nextTick } from 'vue';
import { cmd, subscribe } from '../lib/tauri';
import { hasTauri } from '../lib/hasTauri';
import { DEFAULT_WEB_PORT } from '../lib/const';
import { useToast } from './useToast';
import { modelPath } from './useEnvModels';
import { advanced, aliases } from './useAdvanced';
import { webPort as configuredPort, serverDefaultWebPort } from './useConfig';

// ── Module-level server state (shared by every component that reads it) ──
// The original app kept all refs in one setup(); this mirrors that so
// Sidebar and LaunchView observe the same running/pid/webPort/logs.
const running = ref(false);
const pid = ref<string | null>(null);
const livePort = ref(DEFAULT_WEB_PORT);
const logs = ref<string[]>([]);

// Effective configured port: saved value, else llama-server's detected
// default (custom builds often differ from 8080), else the launcher fallback.
const configuredWebPort = computed<number>(
  () =>
    configuredPort.value ?? serverDefaultWebPort.value ?? DEFAULT_WEB_PORT,
);
const webPort = computed<number>(
  () => (running.value ? livePort.value : configuredWebPort.value),
);
// Saved port differs from the running one: a restart is required.
const portChangePending = computed<boolean>(
  () =>
    running.value &&
    configuredPort.value != null &&
    livePort.value !== configuredWebPort.value,
);
function scrollLog() {
  nextTick(() => {
    const el = document.querySelector<HTMLElement>('.terminal-output');
    if (el) el.scrollTop = el.scrollHeight;
  });
}
watch(logs, scrollLog);

async function syncStatus() {
  if (!hasTauri()) return;
  try {
    const [isRunning, serverPid, port] = await cmd.get_status();
    running.value = !!isRunning;
    pid.value = serverPid != null ? String(serverPid) : null;
    livePort.value = port || DEFAULT_WEB_PORT;
  } catch {
    /* ignore */
  }
}

function buildArgs(): string[] {
  const args: string[] = ['-m', modelPath.value];
  const a = advanced.value;
  if (a.ctx_size != null) args.push('-c', String(a.ctx_size));
  if (a.ngl != null) args.push('-ngl', String(a.ngl));
  const alias = aliases.value[modelPath.value]?.trim();
  if (alias) args.push('--alias', alias);
  const key = a.api_key?.trim();
  if (key) args.push('--api-key', key);
  return args;
}

async function startServer() {
  if (!hasTauri()) {
    useToast().showToast('服务启动仅支持 Tauri 打包客户端', 'error', 4000);
    return;
  }
  running.value = true;
  logs.value = [];
  useToast().showToast('正在启动 llama-server …', 'info', 2000);
  try {
    const args = buildArgs();
    const modelAlias = aliases.value[modelPath.value]?.trim() || '';
    const apiKey = advanced.value.api_key?.trim() || null;
    const serverPid = await cmd.start_server(args, modelAlias, apiKey);
    pid.value = serverPid;
    // Pick up the port the backend actually used (configured/default), even
    // if the server-status event raced with this promise.
    await syncStatus();
    logs.value.push(`✅ 服务器已启动 (PID: ${serverPid})`);
    useToast().showToast(`服务器已启动 (PID: ${serverPid})`, 'success');
  } catch (err) {
    logs.value.push(`❌ 启动失败: ${err}`);
    useToast().showToast(`启动失败: ${err}`, 'error', 5000);
    running.value = false;
  }
}

async function stopServer() {
  if (!hasTauri()) return;
  try {
    const msg = await cmd.stop_server();
    logs.value.push(`⏹ ${msg}`);
    useToast().showToast(msg, 'info');
  } catch (err) {
    useToast().showToast(`停止失败: ${err}`, 'error');
  }
  running.value = false;
  pid.value = null;
}

function clearLog() {
  logs.value = [];
}

async function openWebUI() {
  if (!hasTauri()) return;
  if (!running.value) {
    useToast().showToast('请先启动服务器', 'warning');
    return;
  }
  await cmd.open_url(`http://127.0.0.1:${webPort.value}`);
}

/**
 * Bind the backend event stream (process-exit / server-status / log-line) and
 * backfill log history. Call once from App.onMounted — the root lives for the
 * whole app lifetime, so these are never unlistened.
 */
export async function bindServerEvents() {
  if (!hasTauri()) return;
  await subscribe('process-exit', () => {
    logs.value.push('⏹ 服务器进程已退出');
    useToast().showToast('服务器已停止', 'info');
  });
  await subscribe<{ running: boolean; pid: number | null; port: number }>('server-status', (e) => {
    running.value = !!e.running;
    pid.value = e.pid != null ? String(e.pid) : null;
    if (e.port) livePort.value = e.port;
  });
  await subscribe<string>('log-line', (line) => {
    logs.value.push(line);
    scrollLog();
  });
  // Backfill once: a webview reload while the server is running would
  // otherwise miss the earlier server-status event and fall back to the
  // configured port. Events only apply incremental updates after this.
  await syncStatus();
  try {
    const history = await cmd.get_log_history();
    if (history.length) {
      logs.value = history;
      scrollLog();
    }
  } catch {
    /* ignore */
  }
}

export function useServer() {
  return {
    running,
    pid,
    webPort,
    livePort,
    configuredWebPort,
    portChangePending,
    logs,
    syncStatus,
    startServer,
    stopServer,
    clearLog,
    openWebUI,
    // Full status helper for the API panel (reads the active alias + key).
    fetchStatusFull: () => cmd.get_status_full(),
  };
}
