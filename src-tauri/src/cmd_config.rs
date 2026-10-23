//! Tauri commands: check_config / save_config — read and merge persisted
//! `AppConfig` (last-selected model, aliases, advanced params, web port).
use std::collections::HashMap;
use tauri::{AppHandle, State};

use crate::config::{self, AdvancedParams};
use crate::state::ConfigWriteLock;

#[tauri::command]
pub fn check_config(app_handle: AppHandle) -> Result<serde_json::Value, String> {
    let cfg = config::read_config(&app_handle)?;
    let current_alias = cfg
        .model_path
        .as_ref()
        .and_then(|m| cfg.model_aliases.get(m).cloned());
    Ok(serde_json::json!({
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
    write_lock: State<'_, ConfigWriteLock>,
    model_path: Option<String>,
    model_aliases: Option<HashMap<String, String>>,
    advanced: Option<AdvancedParams>,
    // `i64` so 0 / 70000 / negatives reach our validation instead of failing
    // serde deserialization with an opaque error.
    web_port: Option<i64>,
) -> Result<(), String> {
    let port = config::validate_web_port(web_port)?;
    // The read-modify-write runs entirely under the write lock, so two
    // concurrent saves cannot overwrite each other's changes.
    config::update_config(&app_handle, &write_lock.0, move |cfg| {
        if let Some(v) = model_path {
            cfg.model_path = Some(v);
        }
        if let Some(m) = model_aliases {
            // Replace, not merge: the UI always sends the complete map, so a
            // merge would make cleared aliases impossible to delete. Empty
            // values are dropped so "reset to filename" really clears them.
            let cleaned: HashMap<String, String> = m
                .into_iter()
                .filter_map(|(k, v)| {
                    let v = v.trim();
                    (!v.is_empty()).then(|| (k, v.to_string()))
                })
                .collect();
            cfg.model_aliases = cleaned;
        }
        if let Some(a) = advanced {
            cfg.advanced = Some(a);
        }
        // `web_port` is nullable: `None` clears the setting back to
        // llama.cpp's default port, matching the UI's "留空则使用默认端口".
        cfg.web_port = port;
    })
}
