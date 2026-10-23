//! Tauri command: list_models — recursively enumerate `.gguf` files under the
//! configured `LLAMA_CPP_MODELS_PATH`, deduplicating via canonical paths so
//! symlink loops (e.g. a link back to an ancestor) don't recurse forever.

use crate::env;

const MAX_MODEL_SCAN_DEPTH: usize = 20;

fn walk_models_recursive(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let mut seen = std::collections::HashSet::new();
    walk_models_recursive_depth(dir, out, 0, &mut seen);
}

fn walk_models_recursive_depth(
    dir: &std::path::Path,
    out: &mut Vec<std::path::PathBuf>,
    depth: usize,
    seen: &mut std::collections::HashSet<std::path::PathBuf>,
) {
    if depth > MAX_MODEL_SCAN_DEPTH {
        return;
    }
    // Canonical-path dedup stops symlink loops (e.g. a link back to an
    // ancestor) from recursing forever.
    let canon = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    if !seen.insert(canon) {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_models_recursive_depth(&path, out, depth + 1, seen);
        } else if path.extension().is_some_and(|e| e == "gguf") {
            out.push(path);
        }
    }
}

pub fn list_models_inner() -> Vec<String> {
    let dir = match env::get_env_var_value(env::ENV_MODELS) {
        Some(p) => std::path::PathBuf::from(p),
        None => return Vec::new(),
    };
    if !dir.is_dir() {
        return Vec::new();
    }

    let mut found = Vec::new();
    walk_models_recursive(&dir, &mut found);
    let mut models: Vec<String> = found
        .into_iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    models.sort();
    models
}

#[tauri::command]
pub async fn list_models() -> Result<Vec<String>, String> {
    // Recursive directory walk can take seconds on large model folders.
    tokio::task::spawn_blocking(list_models_inner)
        .await
        .map_err(|e| format!("读取模型列表失败: {}", e))
}
