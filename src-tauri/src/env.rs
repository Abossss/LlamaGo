//! llama.cpp path / env-var helpers: exe name, `LLAMA_CPP_PATH` /
//! `LLAMA_CPP_MODELS_PATH` lookup, and platform-specific env persistence
//! (Windows: `setx` -> HKCU\Environment; Unix: a key=value file under
//! `$XDG_CONFIG_HOME/llamago/env` or `~/.config/llamago/env`).

use std::path::Path;

#[cfg(not(windows))]
use std::collections::HashMap;
#[cfg(windows)]
use std::process::Command;

pub const ENV_LLAMA_CPP: &str = "LLAMA_CPP_PATH";
pub const ENV_MODELS: &str = "LLAMA_CPP_MODELS_PATH";

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn server_exe_name() -> &'static str {
    if cfg!(windows) {
        "llama-server.exe"
    } else {
        "llama-server"
    }
}

pub fn basename_of(p: &str) -> &str {
    p.rsplit(['\\', '/']).next().unwrap_or(p)
}

/// Resolve a var from the process env first, then the persisted store
/// (Windows env vars set via `setx` in a prior session, or the file store on
/// Unix), falling through to `None` if neither has a non-empty value.
pub fn get_env_var_value(name: &str) -> Option<String> {
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
fn env_store_file() -> Option<std::path::PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        let xdg = std::path::PathBuf::from(&xdg);
        if !xdg.as_os_str().is_empty() {
            return Some(xdg.join("llamago").join("env"));
        }
    }
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from)?;
    if home.as_os_str().is_empty() {
        return None;
    }
    Some(home.join(".config").join("llamago").join("env"))
}

#[cfg(not(windows))]
fn read_env_store() -> HashMap<String, String> {
    let mut map = HashMap::new();
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
    let file = env_store_file()
        .ok_or_else(|| "无法确定用户配置目录（未设置 HOME/XDG_CONFIG_HOME）".to_string())?;
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let mut map = read_env_store();
    map.insert(name.to_string(), value.to_string());
    let mut lines: Vec<String> = map.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
    lines.sort();
    std::fs::write(&file, lines.join("\n")).map_err(|e| e.to_string())
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

/// Persist a user-set path into the long-lived env store, validate it first,
/// then also set it for the current process so this session picks it up
/// immediately (no app restart needed).
pub fn save_env_var(name: &str, value: &str) -> Result<(), String> {
    if name != ENV_LLAMA_CPP && name != ENV_MODELS {
        return Err("未知的环境变量名".into());
    }
    if value.trim().is_empty() {
        return Err("路径不能为空".into());
    }
    validate_env_path(name, value)?;
    persist_user_env(name, value)?;
    // Update current process env so this session picks it up immediately
    std::env::set_var(name, value);
    Ok(())
}
