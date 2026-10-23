//! Small, standalone commands: open a URL in the default browser and export
//! the log buffer through a native save dialog.

use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, FilePath};

#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    open::that(&url).map_err(|e| format!("打开失败: {}", e))
}

/// Save the log buffer to a user-chosen file. Returns the path, or `None`
/// when the user cancels the dialog.
#[tauri::command]
pub fn export_log(
    app: AppHandle,
    content: String,
    default_name: String,
) -> Result<Option<String>, String> {
    let picked = app
        .dialog()
        .file()
        .set_file_name(&default_name)
        .blocking_save_file();
    let Some(FilePath::Path(path)) = picked else {
        return Ok(None);
    };
    std::fs::write(&path, content).map_err(|e| format!("写入日志文件失败: {}", e))?;
    Ok(Some(path.to_string_lossy().into_owned()))
}
