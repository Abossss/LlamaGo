//! Tauri commands: scan_llama_cpp / cancel_scan — recursively look for a
//! directory containing `llama-server` (`.exe` on Windows), first trying the
//! configured `LLAMA_CPP_PATH`, falling back to the current exe's directory
//! and `PATH`. Runs off-thread via `tokio::spawn_blocking` so the UI thread
//! stays responsive; cancellable via a shared `AtomicBool`.
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;
use tauri::Emitter;
use tauri::State;

use crate::env;
use crate::state::ScanControl;
use std::path::{Path, PathBuf};

fn is_valid_llama_dir(dir: &Path) -> bool {
    dir.join(env::server_exe_name()).exists()
}

fn get_scan_targets() -> Vec<PathBuf> {
    let mut targets = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Current exe directory
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let canon = parent
                .canonicalize()
                .unwrap_or_else(|_| parent.to_path_buf());
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
pub async fn scan_llama_cpp(
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
        if let Some(p) = env::get_env_var_value(env::ENV_LLAMA_CPP) {
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
                if cf.load(std::sync::atomic::Ordering::Relaxed) {
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

        if cf.load(std::sync::atomic::Ordering::Relaxed) {
            found.clear();
        }

        let paths: Vec<String> = found
            .into_iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();

        let _ = ah.emit(
            "scan-done",
            serde_json::json!({
                "count": paths.len(),
                "paths": paths,
            }),
        );

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
pub fn cancel_scan(state: State<Mutex<ScanControl>>) {
    if let Ok(ctrl) = state.lock() {
        if let Some(flag) = &ctrl.cancel_flag {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
}
