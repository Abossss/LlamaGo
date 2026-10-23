import { computed, ref, watch, nextTick } from 'vue';
import { cmd, subscribe } from '../lib/tauri';
import type { ProcessExitEvent, ServerStatusEvent } from '../types';
import { hasTauri } from '../lib/hasTauri';
import { DEFAULT_WEB_PORT } from '../lib/const';
import { useToast } from './useToast';
import { execPath, modelPath } from './useEnvModels';
import { advanced, aliases } from './useAdvanced';
import {
  webPort as configuredPort,
  serverDefaultWebPort,
  isValidWebPort,
} from './useConfig';
import { fetchWithTimeout } from '../lib/http';

// ── Module-level server state (shared by every component that reads it) ──
// The original app kept all refs in one setup(); this mirrors that so
// Sidebar and LaunchView observe the same running/ready/pid/webPort/logs.
const running = ref(false);
/** True between pressing "start" and the spawn RPC returning. */
const starting = ref(false);
/** True once llama-server reports that it is listening; the model may still
 * be loading, but `/health` and `/v1/models` are reachable. */
const ready = ref(false);
const pid = ref<string | null>(null);
const livePort = ref(DEFAULT_WEB_PORT);

/** Model id served by the running process (alias, or the model path). */
const serverModelId = ref('');
/** Alias/API key of the *running* process (not the saved config). */
const activeAlias = ref('');
const activeKey = ref<string | null>(null);
/** Details of the last unexpected exit, cleared on the next start. */
const lastExit = ref<ProcessExitEvent | null>(null);

interface LogEntry {
  id: number;
  text: string;
}
const logs = ref<LogEntry[]>([]);
let nextLogId = 0;

/** Whether new log lines should scroll the terminal to the bottom. */
const logFollow = ref(true);

// llama-server can be chatty (one line per request on some builds). Keep a
// bounded buffer so a long session cannot grow the DOM/memory forever; 1500
// plain <div>s need no virtualization.
const MAX_LOG_LINES = 1500;

function pushLog(line: string) {
  logs.value.push({ id: ++nextLogId, text: line });
  if (logs.value.length > MAX_LOG_LINES) {
    logs.value.splice(0, logs.value.length - MAX_LOG_LINES);
  }
}

// Effective configured port: saved value, else llama-server's detected
// default (custom builds often differ from 8080), else the launcher fallback.
// An in-progress invalid input is ignored until it becomes valid again.
const configuredWebPort = computed<number>(() => {
  const saved = configuredPort.value;
  if (isValidWebPort(saved) && saved != null) return saved;
  return serverDefaultWebPort.value ?? DEFAULT_WEB_PORT;
});
const webPort = computed<number>(
  () => (running.value ? livePort.value : configuredWebPort.value),
);
// Saved port differs from the running one: a restart is required.
const portChangePending = computed<boolean>(
  () => running.value && livePort.value !== configuredWebPort.value,
);
const statusLabel = computed(() => {
  if (starting.value) return '启动中…';
  if (running.value && !ready.value) return '加载模型…';
  if (running.value) return '运行中';
  return '已停止';
});

function scrollLog() {
  if (!logFollow.value) return;
  nextTick(() => {
    const el = document.querySelector<HTMLElement>('.terminal-output');
    if (el) el.scrollTop = el.scrollHeight;
  });
}
watch(logs, scrollLog);

function setLogFollow(value: boolean) {
  logFollow.value = value;
  if (value) scrollLog();
}

// ── Argument building / preview ─────────────────────────────────────

/** Split a free-form extra-args string into argv, honoring single/double
 * quotes so paths with spaces survive. */
export function tokenizeArgs(input: string): string[] {
  const out: string[] = [];
  let current = '';
  let quote: '"' | "'" | null = null;
  let started = false;

  for (const ch of input) {
    if (quote) {
      if (ch === quote) quote = null;
      else current += ch;
      continue;
    }
    if (ch === '"' || ch === "'") {
      quote = ch;
      started = true;
      continue;
    }
    if (/\s/.test(ch)) {
      if (started) {
        out.push(current);
        current = '';
        started = false;
      }
      continue;
    }
    current += ch;
    started = true;
  }
  if (started) out.push(current);
  return out;
}

function hasExplicitPort(args: string[]): boolean {
  return args.some(
    (a) => a === '--port' || a === '-p' || a.startsWith('--port='),
  );
}

function buildArgs(): string[] {
  const args: string[] = ['-m', modelPath.value];
  const a = advanced.value;
  // Treat empty/NaN input as "not set": a cleared field must fall back to
  // llama-server's own default, never become `-c ''`.
  if (a.ctx_size != null && Number.isInteger(a.ctx_size) && a.ctx_size >= 1) {
    args.push('-c', String(a.ctx_size));
  }
  if (a.ngl != null && Number.isInteger(a.ngl) && a.ngl >= 0) {
    args.push('-ngl', String(a.ngl));
  }
  const alias = aliases.value[modelPath.value]?.trim();
  if (alias) args.push('--alias', alias);
  if (a.extra_args?.trim()) args.push(...tokenizeArgs(a.extra_args));
  // Pass the port the UI shows explicitly: the backend would otherwise read
  // the last *saved* value, which lags behind an unsaved edit of the field.
  if (!hasExplicitPort(args)) {
    args.push('--port', String(configuredWebPort.value));
  }
  // The API key is passed to the backend separately and exported as
  // LLAMA_API_KEY, never as a command-line argument.
  return args;
}

const launchArgs = computed<string[]>(() => (modelPath.value ? buildArgs() : []));

function quoteForDisplay(arg: string): string {
  return /\s/.test(arg) ? `"${arg}"` : arg;
}

function redactArgs(args: string[]): string[] {
  const out: string[] = [];
  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (arg === '--api-key') {
      out.push(arg, '***');
      i++;
    } else if (arg.startsWith('--api-key=')) {
      out.push('--api-key=***');
    } else {
      out.push(arg);
    }
  }
  return out;
}

/** The command the backend will run (port already included by buildArgs). */
const launchCommand = computed<string>(() => {
  if (!modelPath.value) return '';
  return [
    quoteForDisplay(execPath.value || 'llama-server'),
    ...redactArgs(launchArgs.value).map(quoteForDisplay),
  ].join(' ');
});

// ── Status sync ──────────────────────────────────────────────────────

async function syncStatus() {
  if (!hasTauri()) return;
  try {
    const [isRunning, isReady, serverPid, port] = await cmd.get_status();
    running.value = !!isRunning;
    ready.value = !!isRunning && !!isReady;
    pid.value = serverPid != null ? String(serverPid) : null;
    livePort.value = port || DEFAULT_WEB_PORT;
    if (!isRunning) {
      starting.value = false;
      serverModelId.value = '';
    }
  } catch {
    /* ignore */
  }
}

let modelReqToken = 0;

/** Ask the running server for the model id it actually serves. */
async function fetchServerModelId() {
  const token = ++modelReqToken;
  if (!running.value) {
    serverModelId.value = '';
    return;
  }
  // Readiness normally arrives first; the retry loop still covers a missed
  // log line or a slow first request.
  for (let attempt = 0; attempt < 30; attempt++) {
    if (token !== modelReqToken || !running.value) return;
    try {
      const headers: Record<string, string> = {};
      const key = activeKey.value?.trim();
      if (key) headers.Authorization = `Bearer ${key}`;
      const res = await fetchWithTimeout(
        `http://127.0.0.1:${livePort.value}/v1/models`,
        { headers },
        2500,
      );
      if (res.ok) {
        const body = await res.json();
        const id = body?.data?.[0]?.id;
        if (token === modelReqToken && typeof id === 'string' && id) {
          serverModelId.value = id;
        }
        return;
      }
    } catch {
      /* server not accepting requests yet */
    }
    await new Promise((r) => setTimeout(r, 1000));
  }
}

/** Refresh the run metadata (alias/key) and the served model id. */
async function refreshServerModelInfo() {
  if (!hasTauri()) return;
  try {
    const s = await cmd.get_status_full();
    running.value = !!s.running;
    ready.value = !!s.ready && !!s.running;
    pid.value = s.pid != null ? String(s.pid) : null;
    if (s.port) livePort.value = s.port;
    activeAlias.value = s.model_alias || '';
    activeKey.value = s.api_key ?? null;
  } catch {
    return;
  }
  await fetchServerModelId();
}

// ── Actions ─────────────────────────────────────────────────────────

async function startServer() {
  if (!hasTauri()) {
    useToast().showToast('服务启动仅支持 Tauri 打包客户端', 'error', 4000);
    return;
  }
  if (running.value || starting.value) return;

  running.value = true;
  starting.value = true;
  ready.value = false;
  lastExit.value = null;
  serverModelId.value = '';
  activeAlias.value = '';
  activeKey.value = '';
  logs.value = [];
  logFollow.value = true;
  useToast().showToast('正在启动 llama-server …', 'info', 2000);
  try {
    const args = buildArgs();
    const modelAlias = aliases.value[modelPath.value]?.trim() || '';
    const apiKey = advanced.value.api_key?.trim() || null;
    // Reflect the run metadata immediately; the API panel should not claim
    // "no key" while the model is still loading.
    activeAlias.value = modelAlias;
    activeKey.value = apiKey;
    const serverPid = await cmd.start_server(args, modelAlias, apiKey);
    pid.value = serverPid;
    // Pick up the port the backend actually used (configured/default), even
    // if the server-status event raced with this promise.
    await syncStatus();
    pushLog(`✅ 服务器进程已启动 (PID: ${serverPid})，正在加载模型…`);
    useToast().showToast(`服务器已启动 (PID: ${serverPid})`, 'success');
  } catch (err) {
    pushLog(`❌ 启动失败: ${err}`);
    useToast().showToast(`启动失败: ${err}`, 'error', 5000);
    running.value = false;
    ready.value = false;
  } finally {
    starting.value = false;
  }
}

async function stopServer(silent = false) {
  if (!hasTauri()) return;
  try {
    const msg = await cmd.stop_server();
    pushLog(`⏹ ${msg}`);
    if (!silent) useToast().showToast(msg, 'info');
  } catch (err) {
    useToast().showToast(`${silent ? '重启失败' : '停止失败'}: ${err}`, 'error');
  }
  running.value = false;
  starting.value = false;
  ready.value = false;
  pid.value = null;
  serverModelId.value = '';
  activeAlias.value = '';
  activeKey.value = '';
}

/** Stop (if needed) and start again with the current saved parameters. */
async function restartServer() {
  if (!hasTauri()) return;
  if (!running.value) {
    await startServer();
    return;
  }
  useToast().showToast('正在重启服务器…', 'info', 2000);
  await stopServer(true);
  await startServer();
}

function clearLog() {
  logs.value = [];
  lastExit.value = null;
}

async function openWebUI() {
  if (!hasTauri()) return;
  if (!running.value) {
    useToast().showToast('请先启动服务器', 'warning');
    return;
  }
  if (!ready.value) {
    useToast().showToast('模型仍在加载，请稍候再打开 WebUI', 'warning');
    return;
  }
  await cmd.open_url(`http://127.0.0.1:${webPort.value}`);
}

// ── Backend event stream ────────────────────────────────────────────

/**
 * Bind the backend event stream (process-exit / server-status / log-line) and
 * backfill log history. Call once from App.onMounted — the root lives for the
 * whole app lifetime, so these are never unlistened.
 */
export async function bindServerEvents() {
  if (!hasTauri()) return;
  await subscribe<ProcessExitEvent>('process-exit', (e) => {
    starting.value = false;
    running.value = false;
    ready.value = false;
    pid.value = null;
    serverModelId.value = '';
    activeAlias.value = '';
    activeKey.value = '';
    const code = e?.code ?? null;
    lastExit.value = { code, success: !!e?.success };
    const suffix = code != null ? `（退出码 ${code}）` : '';
    pushLog(`❌ llama-server 意外退出${suffix}`);
    useToast().showToast(
      `llama-server 意外退出${suffix}，请查看日志`,
      'error',
      6000,
    );
  });
  await subscribe<ServerStatusEvent>('server-status', (e) => {
    running.value = !!e.running;
    ready.value = !!e.running && !!e.ready;
    pid.value = e.pid != null ? String(e.pid) : null;
    if (e.port) livePort.value = e.port;
    if (!e.running) {
      starting.value = false;
      serverModelId.value = '';
      activeAlias.value = '';
      activeKey.value = '';
    } else {
      // Also covers tray-initiated starts, where the UI never saw the args.
      void refreshServerModelInfo();
    }
  });
  await subscribe<number>('server-ready', (serverPid) => {
    ready.value = true;
    starting.value = false;
    if (serverPid != null) pid.value = String(serverPid);
    pushLog('✅ 服务器已就绪，可以打开 WebUI 或调用 API');
    useToast().showToast('模型加载完成，服务器已就绪', 'success');
    void refreshServerModelInfo();
  });
  await subscribe<string>('log-line', (line) => {
    pushLog(line);
    scrollLog();
  });
  // Backfill once: a webview reload while the server is running would
  // otherwise miss the earlier server-status event and fall back to the
  // configured port. Events only apply incremental updates after this.
  await syncStatus();
  if (running.value) await refreshServerModelInfo();
  try {
    const history = await cmd.get_log_history();
    if (history.length) {
      logs.value = history
        .slice(-MAX_LOG_LINES)
        .map((text) => ({ id: ++nextLogId, text }));
      scrollLog();
    }
  } catch {
    /* ignore */
  }
}

export function useServer() {
  return {
    running,
    starting,
    ready,
    pid,
    webPort,
    livePort,
    configuredWebPort,
    portChangePending,
    statusLabel,
    logs,
    logFollow,
    setLogFollow,
    serverModelId,
    activeAlias,
    activeKey,
    lastExit,
    launchArgs,
    launchCommand,
    tokenizeArgs,
    syncStatus,
    refreshServerModelInfo,
    startServer,
    stopServer,
    restartServer,
    clearLog,
    openWebUI,
  };
}
