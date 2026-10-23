//! System tray menu, window show/hide, and shared emit helpers (toast,
//! server-status, log-line, debug.log). Split out of the original
//! single-file `main.rs`.
use tauri::menu::MenuItem;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};

use crate::server;
use crate::state::AppStateLock;

pub const TRAY_STATUS_ID: &str = "status";
pub const TRAY_TOGGLE_ID: &str = "toggle";
pub const TRAY_WEBUI_ID: &str = "webui";
pub const TRAY_SHOW_ID: &str = "show";
pub const TRAY_QUIT_ID: &str = "quit";

/// Tray menu items are held in Tauri state so their labels/enable-state can
/// be updated at runtime as the server starts/stops.
pub struct TrayItems {
    pub status: MenuItem<Wry>,
    pub toggle: MenuItem<Wry>,
    pub webui: MenuItem<Wry>,
}

// ── Emit helpers ──────────────────────────────────────────────────────

pub fn emit_toast(app: &AppHandle, message: &str, kind: &str) {
    let _ = app.emit(
        "toast",
        serde_json::json!({ "message": message, "type": kind }),
    );
}

pub fn emit_server_status(app: &AppHandle) {
    let (running, pid, port) = {
        let state = app.state::<AppStateLock>();
        let locked = state.lock();
        match locked {
            Ok(lock) => (lock.is_running, lock.server_pid, lock.web_port),
            Err(_) => return,
        }
    };
    let _ = app.emit(
        "server-status",
        serde_json::json!({
            "running": running,
            "pid": pid,
            "port": port,
        }),
    );
}

/// Internal diagnostics: append to `debug.log`, mirror to the terminal view
/// and the log-history buffer. `eprintln!` would be lost on Windows (no
/// console).
pub fn write_log(app: &AppHandle, level: &str, msg: &str) {
    let line = format!("[{}] {}", level, msg);
    if let Ok(dir) = crate::config::get_config_dir(app) {
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("debug.log"))
        {
            use std::io::Write;
            let _ = writeln!(f, "{}", line);
        }
    }
    let _ = app.emit("log-line", &line);
    if let Ok(mut s) = app.state::<AppStateLock>().lock() {
        s.log_history.push_back(line);
        while s.log_history.len() > crate::state::LOG_HISTORY_LIMIT {
            s.log_history.pop_front();
        }
    }
}

// ── Tray menu handlers ───────────────────────────────────────────────

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn stop_server_now(app: &AppHandle) {
    server::stop_server_now(app);
}

pub fn toggle_server_from_tray(app: &AppHandle) {
    let state = app.state::<AppStateLock>();
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
        server::start_server_from_tray(app);
    }
    update_tray_menu(app);
}

/// Keep the tray menu's status line / toggle label / WebUI enabled-state in
/// sync with the real process state.
pub fn update_tray_menu(app: &AppHandle) {
    let items = app.state::<TrayItems>();
    let (running, pid) = {
        let state = app.state::<AppStateLock>();
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
        write_log(
            app,
            "error",
            &format!("更新托盘菜单 {} 失败: {}", TRAY_STATUS_ID, e),
        );
    }
    if let Err(e) = items.toggle.set_text(if running {
        "停止服务器"
    } else {
        "启动服务器"
    }) {
        write_log(
            app,
            "error",
            &format!("更新托盘菜单 {} 标题失败: {}", TRAY_TOGGLE_ID, e),
        );
    }
    if let Err(e) = items.toggle.set_enabled(true) {
        write_log(
            app,
            "error",
            &format!("更新托盘菜单 {} 状态失败: {}", TRAY_TOGGLE_ID, e),
        );
    }
    if let Err(e) = items.webui.set_enabled(running) {
        write_log(
            app,
            "error",
            &format!("更新托盘菜单 {} 状态失败: {}", TRAY_WEBUI_ID, e),
        );
    }
}

/// Build the tray menu + icon once, at app startup (called from `setup`).
pub fn build_tray(app: &tauri::App<Wry>) -> tauri::Result<()> {
    let status_item =
        MenuItem::with_id(app, TRAY_STATUS_ID, "服务器: 已停止", false, None::<&str>)?;
    let toggle_item = MenuItem::with_id(app, TRAY_TOGGLE_ID, "启动服务器", true, None::<&str>)?;
    let webui_item = MenuItem::with_id(app, TRAY_WEBUI_ID, "打开 WebUI", false, None::<&str>)?;
    let show_item = MenuItem::with_id(app, TRAY_SHOW_ID, "显示主窗口", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, TRAY_QUIT_ID, "退出", true, None::<&str>)?;

    let menu = tauri::menu::Menu::with_items(
        app,
        &[
            &status_item,
            &toggle_item,
            &webui_item,
            &tauri::menu::PredefinedMenuItem::separator(app)?,
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
            TRAY_WEBUI_ID => match server::web_url(app) {
                Some(url) => {
                    let _ = crate::misc::open_url(url);
                }
                // The item is disabled while stopped; this also covers the
                // race where the server exits right before the click.
                None => emit_toast(app, "请先启动服务器", "warning"),
            },
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

    let handle = app.handle();
    handle.manage(TrayItems {
        status: status_item,
        toggle: toggle_item,
        webui: webui_item,
    });

    update_tray_menu(&handle);

    Ok(())
}
