//! LlamaGo — Tauri desktop launcher for llama.cpp's `llama-server`.
//!
//! Backend modules (split out from the original single-file implementation):
//!   - `config`       persisted app settings (`config.json` in the app data dir)
//!   - `env`          `LLAMA_CPP_PATH` / `LLAMA_CPP_MODELS_PATH` resolution
//!   - `cmd_config`   `check_config` / `save_config` commands
//!   - `cmd_env`      `get_env_settings` / `save_env_var` / `pick_folder`
//!   - `misc`         `open_url`
//!   - `scan`         `scan_llama_cpp` / `cancel_scan`
//!   - `models`       `list_models`
//!   - `server`       `start_server` / `stop_server` / `get_status` / ... +
//!                    log relay + exit watcher
//!   - `tray`         system-tray menu, show/hide, shared emit helpers
//!   - `state`        shared `Mutex<AppState>` / `ScanControl` + constants
//!
//! This file only wires them together: single-instance guard, Tauri state,
//! window-close interception, tray setup, and the `generate_handler!` list.
#![windows_subsystem = "windows"]

mod cmd_config;
mod cmd_env;
mod config;
mod env;
mod misc;
mod models;
mod scan;
mod server;
mod state;
mod tray;

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

use tauri::WindowEvent;

const SINGLE_INSTANCE_MUTEX: &str = "Local\\LlamaGo.SingleInstance";

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
            tray::build_tray(app)?;
            Ok(())
        })
        .manage(std::sync::Mutex::new(state::AppState::new()))
        .manage(std::sync::Mutex::new(state::ScanControl::new()))
        .manage(state::ConfigWriteLock::new())
        .invoke_handler(tauri::generate_handler![
            cmd_config::check_config,
            cmd_config::save_config,
            cmd_env::get_env_settings,
            cmd_env::save_env_var,
            cmd_env::pick_folder,
            scan::scan_llama_cpp,
            scan::cancel_scan,
            models::list_models,
            server::get_log_history,
            server::start_server,
            server::stop_server,
            server::get_status,
            server::get_status_full,
            misc::open_url,
        ])
        .run(tauri::generate_context!())
        .expect("启动 LlamaGo 时出错");
}
