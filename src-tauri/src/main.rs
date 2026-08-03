#![windows_subsystem = "windows"]

use std::collections::VecDeque;
use std::io::{BufRead, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, Wry, WindowEvent};
use tauri_plugin_dialog::DialogExt;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

const ENV_LLAMA_CPP: &str = "LLAMA_CPP_PATH";
const ENV_MODELS: &str = "LLAMA_CPP_MODELS_PATH";
const LOG_HISTORY_LIMIT: usize = 200;
const DEFAULT_WEB_PORT: u16 = 8080;

const SINGLE_INSTANCE_MUTEX: &str = "Local\\LlamaGo.SingleInstance";

const TRAY_STATUS_ID: &str = "status";
const TRAY_TOGGLE_ID: &str = "toggle";
const TRAY_WEBUI_ID: &str = "webui";
const TRAY_SHOW_ID: &str = "show";
const TRAY_QUIT_ID: &str = "quit";

// ── Single instance ───────────────────────────────────────────────────

#[cfg(windows)]
mod single_instance {
    use std::os::windows::io::RawHandle;
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, SetLastError,
    };
    use windows_sys::Win32::System::Threading::CreateMutexW;

    pub struct Guard(RawHandle);

    impl Guard {
        /// Acquire a session-scoped named mutex. `Some` on the first instance,
        /// `None` when another instance already holds the same mutex.
        pub fn try_acquire(name: &str) -> Option<Guard> {
            let mut wide: Vec<u16> = name.encode_utf16().collect();
            wide.push(0);
            unsafe { SetLastError(0) };
            let handle = unsafe { CreateMutexW(std::ptr::null(), 0, wide.as_ptr()) };
            if handle.is_null() {
                return None;
            }
            if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
                unsafe { CloseHandle(handle) };
                return None;
            }
            Some(Guard(handle))
        }
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0) };
        }
    }
}

// ── App State ─────────────────────────────────────────────────────────

struct AppState {
    is_running: bool,
    server_pid: Option<u32>,
    web_port: u16,
    process: Option<Child>,
    log_history: VecDeque<String>,
}

struct ScanControl {
    cancel_flag: Option<Arc<AtomicBool>>,
}

// Tray menu items are held here so their labels/state can be updated at runtime.
struct TrayItems {
    status: MenuItem<Wry>,
    toggle: MenuItem<Wry>,
    webui: MenuItem<Wry>,
}

// ── Config (last selection) ───────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
struct AppConfig {
    exec_path: Option<String>,
    model_path: Option<String>,
}

fn get_config_dir(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let mut dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    dir.push("LlamaGo");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn read_config(app_handle: &AppHandle) -> AppConfig {
    let dir = match get_config_dir(app_handle) {
        Ok(d) => d,
        Err(_) => return AppConfig::default(),
    };
    let content = match std::fs::read_to_string(dir.join("config.json")) {
        Ok(c) => c,
        Err(_) => return AppConfig::default(),
    };
    serde_json::from_str(&content).unwrap_or_default()
}

fn write_config(app_handle: &AppHandle, config: &AppConfig) -> Result<(), String> {
    let dir = get_config_dir(app_handle)?;
    let content = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("config.json"), content).map_err(|e| e.to_string())
}

fn config_path_str(app_handle: &AppHandle) -> String {
    get_config_dir(app_handle)
        .map(|p| p.join("config.json").to_string_lossy().into_owned())
        .unwrap_or_default()
}

// ── Config Commands ───────────────────────────────────────────────────

#[tauri::command]
fn check_config(app_handle: AppHandle) -> Result<serde_json::Value, String> {
    let cfg = read_config(&app_handle);
    Ok(serde_json::json!({
        "exec_path": cfg.exec_path,
        "model_path": cfg.model_path,
        "config_path": config_path_str(&app_handle),
    }))
}

#[tauri::command]
fn save_config(
    app_handle: AppHandle,
    exec_path: Option<String>,
    model_path: Option<String>,
) -> Result<(), String> {
    let mut cfg = read_config(&app_handle);
    if let Some(v) = exec_path {
        cfg.exec_path = Some(v);
    }
    if let Some(v) = model_path {
        cfg.model_path = Some(v);
    }
    write_config(&app_handle, &cfg)
}

// ── Environment Variables ─────────────────────────────────────────────

fn server_exe_name() -> &'static str {
    if cfg!(windows) { "llama-server.exe" } else { "llama-server" }
}

fn basename_of(p: &str) -> &str {
    p.rsplit(['\\', '/']).next().unwrap_or(p)
}

fn get_env_var_value(name: &str) -> Option<String> {
    if let Some(v) = std::env::var(name).ok().filter(|v| !v.trim().is_empty()) {
        return Some(v);
    }
    env_store_value(name)
}

#[cfg(windows)]
fn env_store_value(_name: &str) -> Option<String> {
    None
}

#[cfg(not(windows))]
fn env_store_value(name: &str) -> Option<String> {
    read_env_store()
        .get(name)
        .cloned()
        .filter(|v| !v.trim().is_empty())
}

#[tauri::command]
fn get_env_settings() -> Result<serde_json::Value, String> {
    let llama = get_env_var_value(ENV_LLAMA_CPP);
    let models = get_env_var_value(ENV_MODELS);
    Ok(serde_json::json!({
        "llama_cpp_path": llama,
        "llama_valid": llama.as_deref()
            .map(|p| Path::new(p).join(server_exe_name()).exists())
            .unwrap_or(false),
        "models_path": models,
        "models_valid": models.as_deref().map(|p| Path::new(p).is_dir()).unwrap_or(false),
    }))
}

#[cfg(windows)]
fn persist_user_env(name: &str, value: &str) -> Result<(), String> {
    // Persist to user environment via setx (writes HKCU\Environment)
    let mut cmd = Command::new("setx");
    cmd.creation_flags(CREATE_NO_WINDOW);
    let status = cmd
        .arg(name)
        .arg(value)
        .status()
        .map_err(|e| format!("写入环境变量失败: {}", e))?;
    if !status.success() {
        return Err("写入环境变量失败".into());
    }
    Ok(())
}

#[cfg(not(windows))]
fn env_store_file() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        let xdg = PathBuf::from(&xdg);
        if !xdg.as_os_str().is_empty() {
            return Some(xdg.join("llamago").join("env"));
        }
    }
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    if home.as_os_str().is_empty() {
        return None;
    }
    Some(home.join(".config").join("llamago").join("env"))
}

#[cfg(not(windows))]
fn read_env_store() -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    if let Some(file) = env_store_file() {
        if let Ok(content) = std::fs::read_to_string(&file) {
            // Split on the FIRST '=' only, so values containing '='
            // (e.g. paths with '=' in them) are preserved intact.
            for line in content.lines() {
                if let Some(eq) = line.find('=') {
                    let key = line[..eq].trim().to_string();
                    let value = line[eq + 1..].trim().to_string();
                    if !key.is_empty() {
                        map.insert(key, value);
                    }
                }
            }
        }
    }
    map
}

#[cfg(not(windows))]
fn write_env_store(name: &str, value: &str) -> Result<(), String> {
    let file = env_store_file().ok_or_else(|| {
        "无法确定用户配置目录（未设置 HOME/XDG_CONFIG_HOME）".to_string()
    })?;
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut map = read_env_store();
    map.insert(name.to_string(), value.to_string());
    let mut lines: Vec<String> = map
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect();
    lines.sort();
    std::fs::write(file, lines.join("\n")).map_err(|e| e.to_string())
}

#[cfg(not(windows))]
fn persist_user_env(name: &str, value: &str) -> Result<(), String> {
    write_env_store(name, value)
}

fn validate_env_path(name: &str, value: &str) -> Result<(), String> {
    let p = Path::new(value);
    if value.contains('\n') || value.contains('\r') {
        return Err("路径不能包含换行符".into());
    }
    if !p.exists() {
        return Err("路径不存在".into());
    }
    if !p.is_dir() {
        return Err("路径不是文件夹".into());
    }
    if name == ENV_LLAMA_CPP && !p.join(server_exe_name()).exists() {
        return Err(format!("目录中未找到 {}", server_exe_name()));
    }
    Ok(())
}

#[tauri::command]
fn save_env_var(name: String, value: String) -> Result<(), String> {
    if name != ENV_LLAMA_CPP && name != ENV_MODELS {
        return Err("未知的环境变量名".into());
    }
    if value.trim().is_empty() {
        return Err("路径不能为空".into());
    }
    validate_env_path(&name, &value)?;
    persist_user_env(&name, &value)?;
    // Update current process env so this session picks it up immediately
    std::env::set_var(&name, &value);
    Ok(())
}

// ── Scan ──────────────────────────────────────────────────────────────

fn is_valid_llama_dir(dir: &Path) -> bool {
    dir.join(server_exe_name()).exists()
}

fn get_scan_targets() -> Vec<PathBuf> {
    let mut targets = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Current exe directory
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let canon = parent.canonicalize().unwrap_or_else(|_| parent.to_path_buf());
            if seen.insert(canon) {
                targets.push(parent.to_path_buf());
            }
        }
    }

    // PATH environment variable directories
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            let canon = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
            if seen.insert(canon) {
                targets.push(dir.to_path_buf());
            }
        }
    }

    targets
}

#[tauri::command]
async fn scan_llama_cpp(
    app_handle: AppHandle,
    state: State<'_, Mutex<ScanControl>>,
) -> Result<Vec<String>, String> {
    let cancel_flag = Arc::new(AtomicBool::new(false));
    {
        let mut ctrl = state.lock().map_err(|e| e.to_string())?;
        ctrl.cancel_flag = Some(cancel_flag.clone());
    }

    let ah = app_handle.clone();
    let cf = cancel_flag.clone();

    let result = tokio::task::spawn_blocking(move || {
        let mut found: Vec<PathBuf> = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // 1) Environment variable first
        if let Some(p) = get_env_var_value(ENV_LLAMA_CPP) {
            let dir = PathBuf::from(p);
            if is_valid_llama_dir(&dir) {
                let canon = dir.canonicalize().unwrap_or_else(|_| dir.clone());
                if seen.insert(canon) {
                    found.push(dir);
                }
            }
        }

        // 2) Fallback: exe dir + PATH
        if found.is_empty() {
            for target in get_scan_targets() {
                if cf.load(Ordering::Relaxed) {
                    break;
                }
                let _ = ah.emit("scan-message", format!("检查: {}", target.display()));
                if is_valid_llama_dir(&target) {
                    let canon = target.canonicalize().unwrap_or_else(|_| target.clone());
                    if seen.insert(canon) {
                        found.push(target);
                    }
                }
            }
        }

        if cf.load(Ordering::Relaxed) {
            found.clear();
        }

        let paths: Vec<String> = found
            .into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();

        let _ = ah.emit("scan-done", serde_json::json!({
            "count": paths.len(),
            "paths": paths,
        }));

        paths
    })
    .await;

    // Always clear the cancel flag so a finished scan never leaks stale state.
    if let Ok(mut ctrl) = state.lock() {
        ctrl.cancel_flag = None;
    }

    let paths = result.map_err(|e| format!("Scan failed: {}", e))?;
    Ok(paths)
}

#[tauri::command]
fn cancel_scan(state: State<Mutex<ScanControl>>) {
    if let Ok(ctrl) = state.lock() {
        if let Some(flag) = &ctrl.cancel_flag {
            flag.store(true, Ordering::Relaxed);
        }
    }
}

// ── Models ────────────────────────────────────────────────────────────

const MAX_MODEL_SCAN_DEPTH: usize = 20;

fn walk_models_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    walk_models_recursive_depth(dir, out, 0, &mut std::collections::HashSet::new());
}

fn walk_models_recursive_depth(
    dir: &Path,
    out: &mut Vec<PathBuf>,
    depth: usize,
    seen: &mut std::collections::HashSet<PathBuf>,
) {
    if depth > MAX_MODEL_SCAN_DEPTH {
        return;
    }
    // Canonical-path dedup stops symlink loops (e.g. a link back to an
    // ancestor) from recursing forever.
    let canon = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    if !seen.insert(canon) {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_models_recursive_depth(&path, out, depth + 1, seen);
        } else if path.extension().is_some_and(|e| e == "gguf") {
            out.push(path);
        }
    }
}

fn list_models_inner() -> Vec<String> {
    let dir = match get_env_var_value(ENV_MODELS) {
        Some(p) => PathBuf::from(p),
        None => return Vec::new(),
    };
    if !dir.is_dir() {
        return Vec::new();
    }

    let mut found = Vec::new();
    walk_models_recursive(&dir, &mut found);
    let mut models: Vec<String> = found
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    models.sort();
    models
}

#[tauri::command]
fn list_models() -> Result<Vec<String>, String> {
    Ok(list_models_inner())
}

// ── Log history ───────────────────────────────────────────────────────

fn push_log_history(app: &AppHandle, line: &str) {
    if let Ok(mut s) = app.state::<Mutex<AppState>>().lock() {
        s.log_history.push_back(line.to_string());
        while s.log_history.len() > LOG_HISTORY_LIMIT {
            s.log_history.pop_front();
        }
    }
}

#[tauri::command]
fn get_log_history(state: State<'_, Mutex<AppState>>) -> Vec<String> {
    state
        .lock()
        .map(|s| s.log_history.iter().cloned().collect())
        .unwrap_or_default()
}

// Emit a server log line and watch for bind failures (the port pre-check in
// start_server_inner is best-effort and racy, so we also react to the actual
// llama-server error output here).
fn relay_server_log(app: &AppHandle, line: &str) {
    let _ = app.emit("log-line", line);
    push_log_history(app, line);
    let lower = line.to_ascii_lowercase();
    if lower.contains("address already in use")
        || lower.contains("bind() failed")
        || lower.contains("listen() failed")
        || lower.contains("unable to bind")
    {
        emit_toast(
            app,
            &format!("端口可能被占用，llama-server 未能启动：{}", line),
            "error",
        );
    }
}

// ── Web port ──────────────────────────────────────────────────────────

fn parse_port_from_args(args: &[String]) -> u16 {
    let mut port = DEFAULT_WEB_PORT;
    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--port" || arg == "-p" {
            if i + 1 < args.len() {
                if let Ok(p) = args[i + 1].parse::<u16>() {
                    if p != 0 {
                        port = p;
                    }
                    // The value is the port token even when it is 0/invalid
                    // as a port, so consume flag + value together.
                    i += 2;
                    continue;
                }
            }
            // Value missing or not a number: only skip the flag itself.
            i += 1;
            continue;
        }
        if let Some(rest) = arg.strip_prefix("--port=") {
            if let Ok(p) = rest.parse::<u16>() {
                if p != 0 {
                    port = p;
                }
            }
        }
        i += 1;
    }
    port
}

fn is_port_in_use(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_err()
}

fn web_url(app: &AppHandle) -> String {
    let port = app
        .state::<Mutex<AppState>>()
        .lock()
        .map(|s| s.web_port)
        .unwrap_or(DEFAULT_WEB_PORT);
    format!("http://127.0.0.1:{}", port)
}

// ── Server ────────────────────────────────────────────────────────────

fn start_server_inner(
    app_handle: &AppHandle,
    state: &Mutex<AppState>,
    executable: &str,
    args: Vec<String>,
) -> Result<String, String> {
    let port = parse_port_from_args(&args);
    if is_port_in_use(port) {
        return Err(format!("端口 {} 已被占用，请使用 --port 参数指定其他端口", port));
    }

    let mut s = state.lock().map_err(|e| e.to_string())?;
    if s.is_running {
        return Err("服务器已在运行中".into());
    }

    let mut cmd = Command::new(executable);
    cmd.args(&args).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("启动失败: {}", e))?;

    let pid = child.id();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    // Only persist state once the process has actually spawned, so a failed
    // start (port busy / spawn error) never leaves a stale web_port behind.
    s.web_port = port;
    s.is_running = true;
    s.server_pid = Some(pid);
    s.process = Some(child);
    drop(s);

    let _ = app_handle.emit("server-started", pid);
    emit_server_status(app_handle);

    let ah_stdout = app_handle.clone();
    thread::spawn(move || {
        let reader = std::io::BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(l) = line {
                relay_server_log(&ah_stdout, &l);
            }
        }
    });

    let ah_stderr = app_handle.clone();
    thread::spawn(move || {
        let reader = std::io::BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(l) = line {
                relay_server_log(&ah_stderr, &format!("[stderr] {}", l));
            }
        }
    });

    let ah_wait = app_handle.clone();
    thread::spawn(move || {
        let state = ah_wait.state::<Mutex<AppState>>();
        loop {
            let decision = {
                let mut lock = match state.lock() {
                    Ok(lock) => lock,
                    Err(_) => return,
                };
                match lock.process.as_mut() {
                    Some(child) => {
                        if child.id() != pid {
                            None
                        } else if child.try_wait().ok().flatten().is_some() {
                            Some(true)
                        } else {
                            Some(false)
                        }
                    }
                    None => None,
                }
            };
            match decision {
                Some(true) => {
                    let should_report = {
                        let mut lock = match state.lock() {
                            Ok(lock) => lock,
                            Err(_) => return,
                        };
                        if lock.process.as_ref().is_some_and(|c| c.id() == pid) {
                            lock.is_running = false;
                            lock.server_pid = None;
                            lock.web_port = DEFAULT_WEB_PORT;
                            lock.process = None;
                            true
                        } else {
                            false
                        }
                    };
                    if should_report {
                        emit_server_status(&ah_wait);
                        let _ = ah_wait.emit("process-exit", ());
                        update_tray_menu(&ah_wait);
                    }
                    return;
                }
                Some(false) => thread::sleep(Duration::from_millis(300)),
                None => return,
            }
        }
    });

    Ok(pid.to_string())
}

#[tauri::command]
fn start_server(
    app_handle: AppHandle,
    state: State<Mutex<AppState>>,
    args: Vec<String>,
) -> Result<String, String> {
    // Executable is always derived from LLAMA_CPP_PATH on the backend, never
    // taken from the frontend, so a tampered frontend cannot launch arbitrary binaries.
    let exec = match get_env_var_value(ENV_LLAMA_CPP) {
        Some(p) => Path::new(&p).join(server_exe_name()).to_string_lossy().into_owned(),
        None => return Err("未配置 LLAMA_CPP_PATH，请先在设置中配置".into()),
    };
    if !Path::new(&exec).exists() {
        return Err(format!("{} 不存在，请检查 LLAMA_CPP_PATH", server_exe_name()));
    }
    let result = start_server_inner(&app_handle, state.inner(), &exec, args);
    update_tray_menu(&app_handle);
    result
}

// Ask the process to stop gracefully, escalating to a hard kill on timeout.
fn terminate_child(child: &mut Child, _pid: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        let _ = Command::new("kill").args(["-TERM", &_pid.to_string()]).status();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            if child.try_wait().map_err(|e| format!("等待进程退出失败: {}", e))?.is_some() {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
        let _ = Command::new("kill").args(["-KILL", &_pid.to_string()]).status();
        let _ = child.kill();
        let _ = child.try_wait();
    }
    #[cfg(windows)]
    {
        child.kill().map_err(|e| format!("终止失败: {}", e))?;
        let _ = child.try_wait();
    }
    Ok(())
}

#[tauri::command]
fn stop_server(app_handle: AppHandle, state: State<Mutex<AppState>>) -> Result<String, String> {
    let result = {
        let mut s = state.lock().map_err(|e| e.to_string())?;
        match s.process.take() {
            Some(mut child) => {
                let pid = child.id();
                terminate_child(&mut child, pid)?;
                s.is_running = false;
                s.server_pid = None;
                s.web_port = DEFAULT_WEB_PORT;
                Ok(format!("已终止进程 {}", pid))
            }
            None => Err("没有运行的服务器".into()),
        }
    };
    if result.is_ok() {
        emit_server_status(&app_handle);
    }
    update_tray_menu(&app_handle);
    result
}

#[tauri::command]
fn get_status(state: State<Mutex<AppState>>) -> Result<(bool, Option<u32>, u16), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok((s.is_running, s.server_pid, s.web_port))
}

// ── Tray ──────────────────────────────────────────────────────────────

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn stop_server_now(app: &AppHandle) {
    if let Ok(mut s) = app.state::<Mutex<AppState>>().lock() {
        if let Some(mut child) = s.process.take() {
            let pid = child.id();
            let _ = terminate_child(&mut child, pid);
        }
        s.is_running = false;
        s.server_pid = None;
        s.web_port = DEFAULT_WEB_PORT;
    }
    emit_server_status(app);
}

fn emit_toast(app: &AppHandle, message: &str, kind: &str) {
    let _ = app.emit("toast", serde_json::json!({ "message": message, "type": kind }));
}

// Internal diagnostics: append to debug.log, mirror to the terminal view and
// the log-history buffer. eprintln! would be lost on Windows (no console).
fn write_log(app: &AppHandle, level: &str, msg: &str) {
    let line = format!("[{}] {}", level, msg);
    if let Ok(dir) = get_config_dir(app) {
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("debug.log"))
        {
            let _ = writeln!(f, "{}", line);
        }
    }
    let _ = app.emit("log-line", &line);
    push_log_history(app, &line);
}

fn emit_server_status(app: &AppHandle) {
    let (running, pid, port) = {
        let state = app.state::<Mutex<AppState>>();
        let locked = state.lock();
        match locked {
            Ok(lock) => (lock.is_running, lock.server_pid, lock.web_port),
            Err(_) => return,
        }
    };
    let _ = app.emit("server-status", serde_json::json!({
        "running": running,
        "pid": pid,
        "port": port,
    }));
}

fn update_tray_menu(app: &AppHandle) {
    let items = app.state::<TrayItems>();
    let (running, pid) = {
        let state = app.state::<Mutex<AppState>>();
        let locked = state.lock();
        match locked {
            Ok(lock) => (lock.is_running, lock.server_pid),
            Err(_) => {
                write_log(app, "error", "状态锁已损坏，无法刷新托盘菜单");
                return;
            }
        }
    };

    let status_title = match pid {
        Some(p) => format!("服务器: 运行中 (PID {})", p),
        None => "服务器: 已停止".to_string(),
    };
    if let Err(e) = items.status.set_text(status_title) {
        write_log(app, "error", &format!("更新托盘菜单 {} 失败: {}", TRAY_STATUS_ID, e));
    }
    if let Err(e) = items.toggle.set_text(if running {
        "停止服务器"
    } else {
        "启动服务器"
    }) {
        write_log(app, "error", &format!("更新托盘菜单 {} 标题失败: {}", TRAY_TOGGLE_ID, e));
    }
    if let Err(e) = items.toggle.set_enabled(true) {
        write_log(app, "error", &format!("更新托盘菜单 {} 状态失败: {}", TRAY_TOGGLE_ID, e));
    }
    if let Err(e) = items.webui.set_enabled(running) {
        write_log(app, "error", &format!("更新托盘菜单 {} 状态失败: {}", TRAY_WEBUI_ID, e));
    }
}

fn tray_start_server(app: &AppHandle) {
    let exec = match get_env_var_value(ENV_LLAMA_CPP) {
        Some(p) => Path::new(&p).join(server_exe_name()).to_string_lossy().into_owned(),
        None => {
            emit_toast(app, "未配置 LLAMA_CPP_PATH，请先在软件中设置", "error");
            return;
        }
    };
    if !Path::new(&exec).exists() {
        emit_toast(app, "llama-server.exe 不存在，请检查 LLAMA_CPP_PATH", "error");
        return;
    }
    let model = match read_config(app).model_path {
        Some(m) if Path::new(&m).exists() => m,
        _ => {
            let candidates = list_models_inner();
            match candidates.first() {
                Some(m) => {
                    let m = m.clone();
                    emit_toast(
                        app,
                        &format!("未保存上次模型选择，已自动使用: {}", basename_of(&m)),
                        "info",
                    );
                    m
                }
                None => {
                    emit_toast(
                        app,
                        "未找到可用的 .gguf 模型，请先打开主窗口配置 LLAMA_CPP_MODELS_PATH 并选择模型",
                        "error",
                    );
                    return;
                }
            }
        }
    };
    let state = app.state::<Mutex<AppState>>();
    match start_server_inner(app, state.inner(), &exec, vec!["-m".into(), model]) {
        Ok(pid) => emit_toast(app, &format!("服务器已启动 (PID: {})", pid), "success"),
        Err(e) => emit_toast(app, &format!("启动失败: {}", e), "error"),
    }
}

fn toggle_server_from_tray(app: &AppHandle) {
    let state = app.state::<Mutex<AppState>>();
    let running = match state.lock() {
        Ok(s) => s.is_running,
        Err(_) => {
            write_log(app, "error", "状态锁已损坏，无法判断服务器状态，已取消操作");
            return;
        }
    };
    if running {
        stop_server_now(app);
        emit_toast(app, "服务器已停止", "info");
    } else {
        tray_start_server(app);
    }
    update_tray_menu(app);
}

// ── Misc commands (dialog / shell replacement) ────────────────────────

#[tauri::command]
fn pick_folder(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::FilePath;
    let picked = app.dialog().file().blocking_pick_folder();
    Ok(match picked {
        Some(FilePath::Path(p)) => Some(p.to_string_lossy().into_owned()),
        Some(FilePath::Url(_)) => None,
        None => None,
    })
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    open::that(&url).map_err(|e| format!("打开失败: {}", e))
}

// ── Main ──────────────────────────────────────────────────────────────

fn main() {
    // If another instance is already running, exit immediately without
    // creating a window or starting any server process.
    #[cfg(windows)]
    let _instance_guard = match single_instance::Guard::try_acquire(SINGLE_INSTANCE_MUTEX) {
        Some(guard) => guard,
        None => return,
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .setup(|app| {
            let status_item = MenuItem::with_id(app, TRAY_STATUS_ID, "服务器: 已停止", false, None::<&str>)?;
            let toggle_item = MenuItem::with_id(app, TRAY_TOGGLE_ID, "启动服务器", true, None::<&str>)?;
            let webui_item = MenuItem::with_id(app, TRAY_WEBUI_ID, "打开 WebUI", false, None::<&str>)?;
            let show_item = MenuItem::with_id(app, TRAY_SHOW_ID, "显示主窗口", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, TRAY_QUIT_ID, "退出", true, None::<&str>)?;

            let menu = Menu::with_items(
                app,
                &[
                    &status_item,
                    &toggle_item,
                    &webui_item,
                    &PredefinedMenuItem::separator(app)?,
                    &show_item,
                    &quit_item,
                ],
            )?;

            let mut tray_builder = TrayIconBuilder::with_id("main")
                .tooltip("LlamaGo")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    TRAY_TOGGLE_ID => toggle_server_from_tray(app),
                    TRAY_WEBUI_ID => {
                        let _ = open_url(web_url(app));
                    }
                    TRAY_SHOW_ID => show_main_window(app),
                    TRAY_QUIT_ID => {
                        stop_server_now(app);
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main_window(tray.app_handle());
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray_builder = tray_builder.icon(icon.clone());
            }
            tray_builder.build(app)?;

            app.manage(TrayItems {
                status: status_item,
                toggle: toggle_item,
                webui: webui_item,
            });

            let handle = app.handle();
            update_tray_menu(&handle);
            Ok(())
        })
        .manage(Mutex::new(AppState {
            is_running: false,
            server_pid: None,
            web_port: DEFAULT_WEB_PORT,
            process: None,
            log_history: VecDeque::new(),
        }))
        .manage(Mutex::new(ScanControl {
            cancel_flag: None,
        }))
        .invoke_handler(tauri::generate_handler![
            start_server,
            stop_server,
            get_status,
            check_config,
            save_config,
            get_env_settings,
            save_env_var,
            scan_llama_cpp,
            cancel_scan,
            list_models,
            get_log_history,
            pick_folder,
            open_url,
        ])
        .run(tauri::generate_context!())
        .expect("启动 LlamaGo 时出错");
}
