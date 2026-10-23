//! `llama-server` process lifecycle: spawn, log relay, exit watcher,
//! graceful/hard stop, and status commands. Split out of the original
//! single-file `main.rs`.
use std::io::BufRead;
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::config;
use crate::env;
use crate::state::{AppState, DEFAULT_WEB_PORT, LOG_HISTORY_LIMIT};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

type StateLock = Mutex<AppState>;

// ── Log history / relay ───────────────────────────────────────────────

fn push_log_history(app: &AppHandle, line: &str) {
    if let Ok(mut s) = app.state::<StateLock>().lock() {
        s.log_history.push_back(line.to_string());
        while s.log_history.len() > LOG_HISTORY_LIMIT {
            s.log_history.pop_front();
        }
    }
}

/// Emit a server log line and watch for bind failures (the port pre-check in
/// `start_server_inner` is best-effort and racy, so we also react to the
/// actual llama-server error output here).
fn relay_server_log(app: &AppHandle, line: &str) {
    let _ = app.emit("log-line", line);
    push_log_history(app, line);
    let lower = line.to_ascii_lowercase();
    if lower.contains("address already in use")
        || lower.contains("bind() failed")
        || lower.contains("listen() failed")
        || lower.contains("unable to bind")
    {
        crate::tray::emit_toast(
            app,
            "端口可能被占用，llama-server 未能启动，请更换 Web 端口",
            "error",
        );
    }
}

#[tauri::command]
pub fn get_log_history(state: State<'_, StateLock>) -> Vec<String> {
    state
        .lock()
        .map(|s| s.log_history.iter().cloned().collect())
        .unwrap_or_default()
}

// ── Web port helpers ──────────────────────────────────────────────────

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

/// Web UI URL for the *running* server. Returns `None` when stopped: opening
/// a URL for a process that isn't listening would always fail.
fn web_url_for(is_running: bool, live_port: u16) -> Option<String> {
    is_running.then(|| format!("http://127.0.0.1:{}", live_port))
}

/// Build the Web UI URL for the current run (used by tray "Open WebUI").
/// Uses the port of the running server, never the configured-but-not-yet
/// applied port.
pub fn web_url(app: &AppHandle) -> Option<String> {
    let state = app.state::<StateLock>();
    let locked = state.lock().ok()?;
    web_url_for(locked.is_running, locked.web_port)
}

// ── Config-derived start (command entry point) ───────────────────────

/// The executable is always derived from `LLAMA_CPP_PATH` on the backend,
/// never taken from the frontend, so a tampered frontend cannot launch
/// arbitrary binaries.
fn resolve_executable() -> Result<String, String> {
    match env::get_env_var_value(env::ENV_LLAMA_CPP) {
        Some(p) => Ok(Path::new(&p)
            .join(env::server_exe_name())
            .to_string_lossy()
            .into_owned()),
        None => Err("未配置 LLAMA_CPP_PATH，请先在设置中配置".into()),
    }
}

#[tauri::command]
pub fn start_server(
    app_handle: AppHandle,
    state: State<StateLock>,
    args: Vec<String>,
    model_alias: Option<String>,
    api_key: Option<String>,
) -> Result<String, String> {
    let exec = resolve_executable()?;
    if !Path::new(&exec).exists() {
        return Err(format!(
            "{} 不存在，请检查 LLAMA_CPP_PATH",
            env::server_exe_name()
        ));
    }

    // Forward the configured Web UI port to llama-server unless the caller
    // already passed an explicit --port (e.g. a future manual-args feature).
    let configured_port = config::read_config(&app_handle).web_port;
    let port = resolve_web_port_for_launch(&args, configured_port, &exec);
    let args = with_configured_web_port(&args, port);

    let result = start_server_inner(
        &app_handle,
        state.inner(),
        &exec,
        args,
        model_alias.as_deref().unwrap_or(""),
        api_key.as_deref(),
    );
    crate::tray::update_tray_menu(&app_handle);
    result
}

/// True when `args` already carries an explicit port flag, in either the
/// separate (`--port 9090` / `-p 9090`) or inline (`--port=9090`) form.
fn has_explicit_web_port(args: &[String]) -> bool {
    args.iter()
        .any(|a| a == "--port" || a == "-p" || a.starts_with("--port="))
}

/// Append `--port <p>` from the saved config to `args` when no explicit
/// `--port`/`-p` flag is already present. `None`/invalid port = leave args
/// untouched. Pure so it can be unit-tested without an `AppHandle`.
fn with_configured_web_port(args: &[String], configured_port: Option<u16>) -> Vec<String> {
    let mut out = args.to_vec();
    if let Some(p) = configured_port.filter(|p| *p >= config::MIN_WEB_PORT) {
        if !has_explicit_web_port(args) {
            out.push("--port".into());
            out.push(p.to_string());
        }
    }
    out
}

/// Pick the port to forward when the caller didn't pass one: saved config
/// first, then the binary's own `--port` default (custom llama.cpp builds may
/// default to something other than 8080), then the launcher fallback.
fn choose_web_port(configured: Option<u16>, detected: Option<u16>) -> u16 {
    configured
        .filter(|p| *p >= config::MIN_WEB_PORT)
        .or(detected.filter(|p| *p >= config::MIN_WEB_PORT))
        .unwrap_or(DEFAULT_WEB_PORT)
}

/// Resolve the `--port` value to append, or `None` when the caller already
/// passed one explicitly.
fn resolve_web_port_for_launch(
    args: &[String],
    configured: Option<u16>,
    executable: &str,
) -> Option<u16> {
    if has_explicit_web_port(args) {
        return None;
    }
    let configured = config::sanitize_web_port(configured);
    Some(choose_web_port(
        configured,
        detect_default_web_port(executable),
    ))
}

/// Parse the default port from `llama-server --help`, e.g.
/// `--port PORT   port to listen (default: 9931)`.
fn parse_default_port_from_help(help: &str) -> Option<u16> {
    help.lines().find_map(|line| {
        let trimmed = line.trim_start();
        let rest = trimmed.strip_prefix("--port")?;
        if !rest.chars().next().is_some_and(char::is_whitespace) {
            return None;
        }
        let idx = trimmed.find("(default:")?;
        let digits: String = trimmed[idx + "(default:".len()..]
            .trim_start()
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        digits
            .parse::<u16>()
            .ok()
            .filter(|p| *p >= config::MIN_WEB_PORT)
    })
}

/// Ask the configured llama-server binary for its default port. Help output
/// is cheap (~60 ms) and avoids assuming upstream's 8080 on custom builds.
fn detect_default_web_port(executable: &str) -> Option<u16> {
    let mut cmd = Command::new(executable);
    cmd.arg("--help").stdin(Stdio::null());
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let output = cmd.output().ok()?;
    parse_default_port_from_help(&String::from_utf8_lossy(&output.stdout))
        .or_else(|| parse_default_port_from_help(&String::from_utf8_lossy(&output.stderr)))
}

/// The configured binary's own default port, for display before the first
/// launch (the Settings page shows the effective Base URL while stopped).
pub fn server_default_web_port() -> Option<u16> {
    let exec = resolve_executable().ok()?;
    if !Path::new(&exec).exists() {
        return None;
    }
    detect_default_web_port(&exec)
}

// ── Core spawn logic (shared with the tray menu) ─────────────────────

pub fn start_server_inner(
    app_handle: &AppHandle,
    state: &Mutex<AppState>,
    executable: &str,
    args: Vec<String>,
    model_alias: &str,
    api_key: Option<&str>,
) -> Result<String, String> {
    let port = parse_port_from_args(&args);
    if is_port_in_use(port) {
        return Err(format!("端口 {} 已被占用，请更换 Web 端口", port));
    }

    let mut s = state.lock().map_err(|e| e.to_string())?;
    if s.is_running {
        return Err("服务器已在运行中".into());
    }

    let mut cmd = Command::new(executable);
    cmd.args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let mut child = cmd.spawn().map_err(|e| format!("启动失败: {}", e))?;

    let pid = child.id();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    // Only persist state once the process has actually spawned, so a failed
    // start (port busy / spawn error) never leaves a stale web_port behind.
    s.web_port = port;
    s.is_running = true;
    s.server_pid = Some(pid);
    s.process = Some(child);
    s.model_alias = model_alias.to_string();
    s.api_key = api_key.map(|k| k.to_string());
    drop(s);

    let _ = app_handle.emit("server-started", pid);
    crate::tray::emit_server_status(app_handle);

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
        let state = ah_wait.state::<StateLock>();
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
                            lock.clear_run();
                            true
                        } else {
                            false
                        }
                    };
                    if should_report {
                        crate::tray::emit_server_status(&ah_wait);
                        let _ = ah_wait.emit("process-exit", ());
                        crate::tray::update_tray_menu(&ah_wait);
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

/// Ask the process to stop gracefully, escalating to a hard kill on timeout.
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
pub fn stop_server(
    app_handle: AppHandle,
    state: State<StateLock>,
) -> Result<String, String> {
    let result = {
        let mut s = state.lock().map_err(|e| e.to_string())?;
        match s.process.take() {
            Some(mut child) => {
                let pid = child.id();
                terminate_child(&mut child, pid)?;
                s.clear_run();
                Ok(format!("已终止进程 {}", pid))
            }
            None => Err("没有运行的服务器".into()),
        }
    };
    if result.is_ok() {
        crate::tray::emit_server_status(&app_handle);
    }
    crate::tray::update_tray_menu(&app_handle);
    result
}

#[tauri::command]
pub fn get_status(state: State<StateLock>) -> Result<(bool, Option<u32>, u16), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(s.status_snapshot())
}

/// Full status for the API panel: adds the active model alias and api_key.
#[tauri::command]
pub fn get_status_full(state: State<StateLock>) -> Result<serde_json::Value, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "running": s.is_running,
        "pid": s.server_pid,
        "port": s.web_port,
        "model_alias": s.model_alias,
        "api_key": s.api_key,
    }))
}

// ── Tray-facing helpers (used by the system-tray menu handlers) ─────

/// Stop the server immediately without going through the full Tauri command
/// wrapper (used by tray "stop" and "quit").
pub fn stop_server_now(app: &AppHandle) {
    if let Ok(mut s) = app.state::<StateLock>().lock() {
        if let Some(mut child) = s.process.take() {
            let pid = child.id();
            let _ = terminate_child(&mut child, pid);
        }
        s.clear_run();
    }
    crate::tray::emit_server_status(app);
}

/// Kick off the server from the tray menu: resolve the last-used model
/// (falling back to the first `.gguf` found, with a toast) and reuse the
/// shared spawn path.
pub fn start_server_from_tray(app: &AppHandle) {
    let exec = match env::get_env_var_value(env::ENV_LLAMA_CPP) {
        Some(p) => Path::new(&p)
            .join(env::server_exe_name())
            .to_string_lossy()
            .into_owned(),
        None => {
            crate::tray::emit_toast(
                app,
                "未配置 LLAMA_CPP_PATH，请先在软件中设置",
                "error",
            );
            return;
        }
    };
    if !Path::new(&exec).exists() {
        crate::tray::emit_toast(app, "llama-server.exe 不存在，请检查 LLAMA_CPP_PATH", "error");
        return;
    }
    let model = match resolve_tray_model(app) {
        Some(m) => m,
        None => {
            crate::tray::emit_toast(
                app,
                "未找到可用的 .gguf 模型，请先打开主窗口配置 LLAMA_CPP_MODELS_PATH 并选择模型",
                "error",
            );
            return;
        }
    };
    let mut args: Vec<String> = vec!["-m".into(), model];
    // Honor the saved Web UI port, same as the command-driven start path.
    let configured_port = config::read_config(app).web_port;
    let port = resolve_web_port_for_launch(&args, configured_port, &exec);
    args = with_configured_web_port(&args, port);
    let state = app.state::<StateLock>();
    match start_server_inner(app, state.inner(), &exec, args, "", None) {
        Ok(pid) => crate::tray::emit_toast(app, &format!("服务器已启动 (PID: {})", pid), "success"),
        Err(e) => crate::tray::emit_toast(app, &format!("启动失败: {}", e), "error"),
    }
}

/// Pick which model the tray "start" flow should use: the last saved
/// selection if it still exists, otherwise the first available model (with a
/// toast explaining the fallback).
fn resolve_tray_model(app: &AppHandle) -> Option<String> {
    let saved = config::read_config(app).model_path;
    if let Some(m) = saved {
        if Path::new(&m).exists() {
            return Some(m);
        }
    }
    crate::models::list_models_inner().into_iter().next().map(|m| {
        crate::tray::emit_toast(
            app,
            &format!("未保存上次模型选择，已自动使用: {}", env::basename_of(&m)),
            "info",
        );
        m
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn appends_configured_port_when_absent() {
        let out = with_configured_web_port(&v(&["-m", "model.gguf"]), Some(9091));
        assert_eq!(out, v(&["-m", "model.gguf", "--port", "9091"]));
    }

    #[test]
    fn keeps_existing_separate_port_flag() {
        let input = v(&["-m", "model.gguf", "--port", "9090"]);
        assert_eq!(with_configured_web_port(&input, Some(8080)), input);
    }

    #[test]
    fn keeps_existing_inline_port_flag() {
        let input = v(&["--port=9090"]);
        assert_eq!(with_configured_web_port(&input, Some(8080)), input);
    }

    #[test]
    fn keeps_existing_short_port_flag() {
        let input = v(&["-p", "9090"]);
        assert_eq!(with_configured_web_port(&input, Some(8080)), input);
    }

    #[test]
    fn leaves_args_untouched_without_configured_port() {
        let input = v(&["-m", "model.gguf"]);
        assert_eq!(with_configured_web_port(&input, None), input);
    }

    #[test]
    fn ignores_invalid_configured_port() {
        let input = v(&["-m", "model.gguf"]);
        assert_eq!(with_configured_web_port(&input, Some(0)), input);
        assert_eq!(with_configured_web_port(&input, Some(80)), input);
    }

    #[test]
    fn parses_port_from_args() {
        assert_eq!(parse_port_from_args(&v(&["-m", "model.gguf"])), DEFAULT_WEB_PORT);
        assert_eq!(parse_port_from_args(&v(&["--port", "9090"])), 9090);
        assert_eq!(parse_port_from_args(&v(&["-p", "9091"])), 9091);
        assert_eq!(parse_port_from_args(&v(&["--port=9092"])), 9092);
        assert_eq!(parse_port_from_args(&v(&["--port", "not-a-port"])), DEFAULT_WEB_PORT);
    }

    #[test]
    fn detects_occupied_port() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        assert!(is_port_in_use(port));
        drop(listener);
        assert!(!is_port_in_use(port));
    }

    #[test]
    fn parses_default_port_from_help() {
        assert_eq!(
            parse_default_port_from_help("--port PORT   port to listen (default: 9931)"),
            Some(9931)
        );
        assert_eq!(
            parse_default_port_from_help("--port PORT   port to listen (default: 8080)"),
            Some(8080)
        );
        assert_eq!(parse_default_port_from_help("no port flag here"), None);
        assert_eq!(
            parse_default_port_from_help("--port-file (default: 0)"),
            None
        );
    }

    #[test]
    fn chooses_config_then_server_default_then_fallback() {
        assert_eq!(choose_web_port(Some(9090), Some(9931)), 9090);
        assert_eq!(choose_web_port(None, Some(9931)), 9931);
        assert_eq!(choose_web_port(Some(80), Some(9931)), 9931);
        assert_eq!(choose_web_port(None, None), DEFAULT_WEB_PORT);
    }

    #[test]
    fn resolve_launch_port_respects_explicit_flag() {
        assert_eq!(
            resolve_web_port_for_launch(&v(&["--port", "9090"]), None, "unused"),
            None
        );
        assert_eq!(
            resolve_web_port_for_launch(&v(&["-m", "m.gguf"]), Some(9090), "unused"),
            Some(9090)
        );
    }

    #[test]
    fn web_url_only_exists_while_running() {
        assert_eq!(
            web_url_for(true, 9090).as_deref(),
            Some("http://127.0.0.1:9090")
        );
        assert_eq!(web_url_for(false, 8080), None);
    }
}
