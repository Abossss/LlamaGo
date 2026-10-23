//! Persisted user configuration: last-selected model/alias/advanced params,
//! written to `config.json` under the app data dir.
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};
use tempfile::NamedTempFile;

use crate::state::DEFAULT_WEB_PORT;

const CONFIG_FILE: &str = "config.json";

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
#[derive(serde::Serialize, serde::Deserialize, Default, Clone, Debug)]
pub struct AdvancedParams {
    pub ctx_size: Option<u32>,
    pub ngl: Option<i32>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub api_key: Option<String>,
}

/// Persisted app config. The executable is not stored: it is always derived
/// from `LLAMA_CPP_PATH`, so a stale config cannot override the environment.
#[derive(serde::Serialize, serde::Deserialize, Default, Clone, Debug)]
pub struct AppConfig {
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

pub fn read_config(app_handle: &AppHandle) -> Result<AppConfig, String> {
    let dir = get_config_dir(app_handle)?;
    read_config_from_path(&dir.join(CONFIG_FILE))
}

/// Read a config file. A missing file means "first run" and yields defaults;
/// a corrupt file is backed up to `config.json.bak.<timestamp>`, recorded in
/// `debug.log`, and reported as an error instead of being silently ignored.
fn read_config_from_path(path: &Path) -> Result<AppConfig, String> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(AppConfig::default());
        }
        Err(e) => return Err(format!("读取配置文件失败: {}", e)),
    };
    serde_json::from_str(&content).map_err(|err| {
        let detail = match backup_corrupt_config(path, &content) {
            Ok(backup) => format!("原文件已备份到 {}", backup.display()),
            Err(e) => format!("备份失败: {}", e),
        };
        let msg = format!("配置文件解析失败: {}（{}）", err, detail);
        append_debug_log(path, &msg);
        msg
    })
}

fn backup_corrupt_config(path: &Path, content: &str) -> Result<PathBuf, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| CONFIG_FILE.to_string());
    let backup = path.with_file_name(format!(
        "{}.bak.{}.{:03}",
        name,
        stamp.as_secs(),
        stamp.subsec_millis()
    ));
    std::fs::write(&backup, content).map_err(|e| e.to_string())?;
    Ok(backup)
}

fn append_debug_log(path: &Path, msg: &str) {
    let Some(dir) = path.parent() else {
        return;
    };
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("debug.log"))
    {
        let _ = writeln!(file, "[error] {}", msg);
    }
}

/// Replace the config file atomically: write a sibling temp file, fsync it,
/// then persist (rename-over) the target. `NamedTempFile::persist` uses the
/// platform's atomic replace on Windows, unlike a hand-written rename.
fn write_config_to_path(path: &Path, config: &AppConfig) -> Result<(), String> {
    let dir = path
        .parent()
        .ok_or_else(|| "配置文件路径无效".to_string())?;
    std::fs::create_dir_all(dir).map_err(|e| format!("创建配置目录失败: {}", e))?;
    let content = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    let mut tmp =
        NamedTempFile::new_in(dir).map_err(|e| format!("创建临时配置文件失败: {}", e))?;
    tmp.write_all(content.as_bytes())
        .and_then(|_| tmp.as_file().sync_all())
        .map_err(|e| format!("写入临时配置文件失败: {}", e))?;
    tmp.persist(path)
        .map_err(|e| format!("替换配置文件失败: {}", e.error))?;
    Ok(())
}

/// Serialize a full read-modify-write cycle under `write_lock`. Callers that
/// run concurrently (e.g. alias edits racing a model switch) can therefore
/// never lose each other's keys.
pub fn update_config(
    app_handle: &AppHandle,
    write_lock: &Mutex<()>,
    update: impl FnOnce(&mut AppConfig),
) -> Result<(), String> {
    let dir = get_config_dir(app_handle)?;
    update_config_at(&dir.join(CONFIG_FILE), write_lock, update)
}

fn update_config_at(
    path: &Path,
    write_lock: &Mutex<()>,
    update: impl FnOnce(&mut AppConfig),
) -> Result<(), String> {
    let _guard = write_lock
        .lock()
        .map_err(|_| "配置写锁已损坏".to_string())?;
    let mut cfg = read_config_from_path(path)?;
    update(&mut cfg);
    write_config_to_path(path, &cfg)
}

/// Human-readable path to `config.json` (or empty string if unresolvable) —
/// surfaced to the UI under 设置 → 配置信息.
pub fn config_path_str(app_handle: &AppHandle) -> String {
    get_config_dir(app_handle)
        .map(|p| p.join(CONFIG_FILE).to_string_lossy().into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_config_without_web_port_defaults_to_8080() {
        let cfg: AppConfig = serde_json::from_str(r#"{"model_path":null}"#).unwrap();
        assert_eq!(cfg.web_port, Some(DEFAULT_WEB_PORT));
        assert_eq!(cfg.web_port, Some(8080));
    }

    #[test]
    fn legacy_exec_path_is_ignored_on_load() {
        // Older configs stored an executable path; deserialization must keep
        // the rest of the config but never resurrect the stale path.
        let cfg: AppConfig = serde_json::from_str(
            r#"{"exec_path":"C:\\old\\llama-server.exe","model_path":"C:\\models\\a.gguf"}"#,
        )
        .unwrap();
        assert_eq!(cfg.model_path.as_deref(), Some("C:\\models\\a.gguf"));
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

    #[test]
    fn missing_config_reads_as_default() {
        let dir = tempfile::TempDir::new().unwrap();
        let cfg = read_config_from_path(&dir.path().join(CONFIG_FILE)).unwrap();
        assert_eq!(cfg.model_path, None);
    }

    #[test]
    fn write_config_replaces_atomically_without_leftovers() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join(CONFIG_FILE);
        std::fs::write(&path, "old contents").unwrap();
        let cfg = AppConfig {
            model_path: Some("C:\\models\\a.gguf".into()),
            web_port: Some(8090),
            ..AppConfig::default()
        };
        write_config_to_path(&path, &cfg).unwrap();

        let loaded = read_config_from_path(&path).unwrap();
        assert_eq!(loaded.model_path.as_deref(), Some("C:\\models\\a.gguf"));
        assert_eq!(loaded.web_port, Some(8090));
        // Only config.json remains; the temp file was persisted over it.
        let names: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec![CONFIG_FILE.to_string()]);
    }

    #[test]
    fn corrupt_config_is_backed_up_logged_and_errors() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join(CONFIG_FILE);
        std::fs::write(&path, "{not json").unwrap();

        let err = read_config_from_path(&path).unwrap_err();
        assert!(err.contains("解析失败"), "unexpected error: {err}");

        let backups: Vec<std::path::PathBuf> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("config.json.bak."))
            })
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(std::fs::read_to_string(&backups[0]).unwrap(), "{not json");

        let debug = std::fs::read_to_string(dir.path().join("debug.log")).unwrap();
        assert!(debug.contains("解析失败"), "debug.log: {debug}");
    }

    #[test]
    fn concurrent_updates_do_not_lose_keys() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join(CONFIG_FILE);
        let lock = Mutex::new(());

        std::thread::scope(|scope| {
            for i in 0..8 {
                let path = &path;
                let lock = &lock;
                scope.spawn(move || {
                    update_config_at(path, lock, |cfg| {
                        cfg.model_aliases
                            .insert(format!("model-{i}"), format!("alias-{i}"));
                    })
                    .unwrap();
                });
            }
        });

        let cfg = read_config_from_path(&path).unwrap();
        assert_eq!(cfg.model_aliases.len(), 8);
    }
}
