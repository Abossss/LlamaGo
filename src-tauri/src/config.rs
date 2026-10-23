//! Persisted user configuration: last-selected model/alias/advanced params,
//! written to `config.json` under the app data dir.
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

use crate::state::DEFAULT_WEB_PORT;

/// Lowest Web port accepted by the UI and persisted config. Ports below 1024
/// are privileged on Unix and prone to colliding with system services.
pub const MIN_WEB_PORT: u16 = 1024;

/// serde default for `web_port`: configs written before the field existed
/// keep working and fall back to llama.cpp's default port.
pub fn default_web_port() -> Option<u16> {
    Some(DEFAULT_WEB_PORT)
}

/// Drop legacy/invalid persisted ports (< 1024) so they never reach the
/// spawn path or the UI.
pub fn sanitize_web_port(port: Option<u16>) -> Option<u16> {
    port.filter(|p| *p >= MIN_WEB_PORT)
}

/// Validate a port supplied by the frontend. `i64` keeps out-of-range values
/// (e.g. 70000 or negatives) inside the command so they can be rejected with
/// a friendly message instead of a serde deserialization error.
pub fn validate_web_port(port: Option<i64>) -> Result<Option<u16>, String> {
    match port {
        None => Ok(None),
        Some(p) if (MIN_WEB_PORT as i64..=u16::MAX as i64).contains(&p) => Ok(Some(p as u16)),
        Some(_) => Err(format!("Web 端口必须在 {}–{} 之间", MIN_WEB_PORT, u16::MAX)),
    }
}

/// Advanced per-model parameters surfaced in the UI. `temperature` and
/// `max_tokens` are recorded but NOT passed to llama-server (OpenAI clients
/// set these per-request); `ctx_size`/`ngl`/`api_key` are forwarded as CLI args.
#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
pub struct AdvancedParams {
    pub ctx_size: Option<u32>,
    pub ngl: Option<i32>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub api_key: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
pub struct AppConfig {
    pub exec_path: Option<String>,
    pub model_path: Option<String>,
    /// Per-model display alias (model path -> alias).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub model_aliases: HashMap<String, String>,
    /// Last-used advanced parameter set (shared across all models).
    #[serde(default)]
    pub advanced: Option<AdvancedParams>,
    /// Web UI port to open after the server starts. Forwarded to
    /// `llama-server` as `--port` on every launch; a missing field (old
    /// config) or `None` falls back to llama.cpp's default port.
    #[serde(default = "default_web_port")]
    pub web_port: Option<u16>,
}

/// Resolve the config directory: `<app_data_dir>/LlamaGo`, created on demand.
pub fn get_config_dir(app_handle: &AppHandle) -> Result<PathBuf, String> {
    let mut dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    dir.push("LlamaGo");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

pub fn read_config(app_handle: &AppHandle) -> AppConfig {
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

pub fn write_config(app_handle: &AppHandle, config: &AppConfig) -> Result<(), String> {
    let dir = get_config_dir(app_handle)?;
    let content = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("config.json"), content).map_err(|e| e.to_string())
}

/// Human-readable path to `config.json` (or empty string if unresolvable) —
/// surfaced to the UI under 设置 → 配置信息.
pub fn config_path_str(app_handle: &AppHandle) -> String {
    get_config_dir(app_handle)
        .map(|p| p.join("config.json").to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_config_without_web_port_defaults_to_8080() {
        let cfg: AppConfig = serde_json::from_str(r#"{"exec_path":null}"#).unwrap();
        assert_eq!(cfg.web_port, Some(DEFAULT_WEB_PORT));
        assert_eq!(cfg.web_port, Some(8080));
    }

    #[test]
    fn explicit_null_web_port_stays_none() {
        let cfg: AppConfig = serde_json::from_str(r#"{"web_port":null}"#).unwrap();
        assert_eq!(cfg.web_port, None);
    }

    #[test]
    fn validate_web_port_accepts_valid_range() {
        assert_eq!(validate_web_port(None), Ok(None));
        assert_eq!(validate_web_port(Some(1024)), Ok(Some(1024)));
        assert_eq!(validate_web_port(Some(8080)), Ok(Some(8080)));
        assert_eq!(validate_web_port(Some(65535)), Ok(Some(65535)));
    }

    #[test]
    fn validate_web_port_rejects_invalid_values() {
        for invalid in [
            Some(0),
            Some(80),
            Some(1023),
            Some(65536),
            Some(70000),
            Some(-1),
        ] {
            assert!(
                validate_web_port(invalid).is_err(),
                "should reject {invalid:?}"
            );
        }
    }

    #[test]
    fn sanitize_web_port_drops_legacy_invalid_values() {
        assert_eq!(sanitize_web_port(Some(0)), None);
        assert_eq!(sanitize_web_port(Some(80)), None);
        assert_eq!(sanitize_web_port(Some(8080)), Some(8080));
        assert_eq!(sanitize_web_port(None), None);
    }
}
