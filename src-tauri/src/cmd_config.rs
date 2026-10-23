//! Tauri commands: check_config / save_config — read and merge persisted
//! `AppConfig` (last-selected model, aliases, advanced params, web port).
use tauri::AppHandle;
use std::collections::HashMap;

use crate::config::{self, AdvancedParams};

#[tauri::command]
pub fn check_config(app_handle: AppHandle) -> Result<serde_json::Value, String> {
    let cfg = config::read_config(&app_handle);
    let current_alias = cfg
        .model_path
        .as_ref()
        .and_then(|m| cfg.model_aliases.get(m).cloned());
    Ok(serde_json::json!({
        "exec_path": cfg.exec_path,
        "model_path": cfg.model_path,
        "config_path": config::config_path_str(&app_handle),
        "model_aliases": cfg.model_aliases,
        "advanced": cfg.advanced,
        // Never surface a legacy out-of-range port to the UI.
        "web_port": config::sanitize_web_port(cfg.web_port),
        // llama-server's own default (custom builds may not use 8080); lets
        // the Settings page show the effective Base URL before first launch.
        "server_default_web_port": crate::server::server_default_web_port(),
        "current_alias": current_alias,
    }))
}

#[tauri::command]
pub fn save_config(
    app_handle: AppHandle,
    exec_path: Option<String>,
    model_path: Option<String>,
    model_aliases: Option<HashMap<String, String>>,
    advanced: Option<AdvancedParams>,
    // `i64` so 0 / 70000 / negatives reach our validation instead of failing
    // serde deserialization with an opaque error.
    web_port: Option<i64>,
) -> Result<(), String> {
    let mut cfg = config::read_config(&app_handle);
    if let Some(v) = exec_path {
        cfg.exec_path = Some(v);
    }
    if let Some(v) = model_path {
        cfg.model_path = Some(v);
    }
    if let Some(m) = model_aliases {
        // Merge: user-supplied entries win, but keep existing aliases for
        // other models so switching model selections doesn't lose them.
        for (k, v) in m {
            if !v.trim().is_empty() {
                cfg.model_aliases.insert(k, v);
            }
        }
    }
    if let Some(a) = advanced {
        cfg.advanced = Some(a);
    }
    // `web_port` is nullable: `None` clears the setting back to llama.cpp's
    // default port, matching the UI's "留空则使用默认端口" affordance.
    // Out-of-range values are rejected instead of being written to disk.
    cfg.web_port = config::validate_web_port(web_port)?;
    config::write_config(&app_handle, &cfg)
}
