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
    Ok(serde_json::json!({
        "llama_cpp_path": llama,
        "llama_valid": llama.as_deref()
            .map(|p| Path::new(p).join(env::server_exe_name()).exists())
            .unwrap_or(false),
        "models_path": models,
        "models_valid": models.as_deref().map(|p| Path::new(p).is_dir()).unwrap_or(false),
    }))
}

#[tauri::command]
pub fn save_env_var(name: String, value: String) -> Result<(), String> {
    env::save_env_var(&name, &value)
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
