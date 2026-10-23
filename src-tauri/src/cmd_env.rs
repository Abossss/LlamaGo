//! Tauri commands: get_env_settings / save_env_var — read or persist
//! `LLAMA_CPP_PATH` / `LLAMA_CPP_MODELS_PATH`; pick_folder — native folder
//! picker.
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

use crate::env;
use std::path::Path;

#[tauri::command]
pub fn get_env_settings() -> Result<serde_json::Value, String> {
    let llama = env::get_env_var_value(env::ENV_LLAMA_CPP);
    let models = env::get_env_var_value(env::ENV_MODELS);
    // Resolve the platform-specific executable server-side so the UI never
    // has to guess the separator or the `.exe` suffix.
    let exec_path = llama
        .as_deref()
        .map(|p| Path::new(p).join(env::server_exe_name()))
        .filter(|p| p.exists())
        .map(|p| p.to_string_lossy().into_owned());
    Ok(serde_json::json!({
        "llama_cpp_path": llama,
        "llama_valid": exec_path.is_some(),
        "exec_path": exec_path,
        "models_path": models,
        "models_valid": models.as_deref().map(|p| Path::new(p).is_dir()).unwrap_or(false),
    }))
}

#[tauri::command]
pub async fn save_env_var(name: String, value: String) -> Result<(), String> {
    // `setx` on Windows can take ~100 ms–1 s; keep it off the runtime thread.
    tokio::task::spawn_blocking(move || env::save_env_var(&name, &value))
        .await
        .map_err(|e| format!("保存环境变量失败: {}", e))?
}

/// Suggest a models folder for `<llama.cpp>`: the release archives ship a
/// `models/` subfolder, while many users keep the binaries in a subfolder and
/// the models one level up (`llama/llama_cpp` + `llama/models`).
#[tauri::command]
pub fn suggest_models_path(llama_dir: String) -> Result<Option<String>, String> {
    let dir = Path::new(llama_dir.trim());
    if !dir.is_dir() {
        return Err("llama.cpp 目录不存在".into());
    }
    Ok(find_models_dir(dir).map(|p| p.to_string_lossy().into_owned()))
}

fn find_models_dir(dir: &Path) -> Option<std::path::PathBuf> {
    let local = dir.join("models");
    if local.is_dir() {
        return Some(local);
    }
    let sibling = dir.parent()?.join("models");
    sibling.is_dir().then_some(sibling)
}

/// Native folder picker; returns `Ok(None)` when the user cancels.
#[tauri::command]
pub fn pick_folder(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::FilePath;
    let picked = app.dialog().file().blocking_pick_folder();
    Ok(match picked {
        Some(FilePath::Path(p)) => Some(p.to_string_lossy().into_owned()),
        Some(FilePath::Url(_)) => None,
        None => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_models_inside_llama_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let llama = tmp.path().join("llama");
        fs::create_dir_all(llama.join("models")).unwrap();
        assert_eq!(find_models_dir(&llama), Some(llama.join("models")));
    }

    #[test]
    fn prefers_local_over_sibling_models_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let llama = tmp.path().join("llama");
        fs::create_dir_all(llama.join("models")).unwrap();
        fs::create_dir_all(tmp.path().join("models")).unwrap();
        assert_eq!(find_models_dir(&llama), Some(llama.join("models")));
    }

    #[test]
    fn finds_sibling_models_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let llama = tmp.path().join("llama_cpp");
        fs::create_dir_all(&llama).unwrap();
        fs::create_dir_all(tmp.path().join("models")).unwrap();
        assert_eq!(find_models_dir(&llama), Some(tmp.path().join("models")));
    }

    #[test]
    fn returns_none_without_models_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let llama = tmp.path().join("llama_cpp");
        fs::create_dir_all(&llama).unwrap();
        assert_eq!(find_models_dir(&llama), None);
    }
}
