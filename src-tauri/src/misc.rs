//! Small, standalone command: "open URL in default browser".

#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    open::that(&url).map_err(|e| format!("打开失败: {}", e))
}
