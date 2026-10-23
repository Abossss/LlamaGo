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
        } else if is_launchable_model(&path) {
            out.push(path);
        }
    }
}

/// A `.gguf` file that can be launched on its own. Vision projectors
/// (`*mmproj*.gguf`) carry no weights, and split-model continuations are
/// useless without the first shard, so neither belongs in the model picker.
fn is_launchable_model(path: &std::path::Path) -> bool {
    if !path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("gguf"))
    {
        return false;
    }
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return true;
    };
    let lower = stem.to_ascii_lowercase();
    !lower.contains("mmproj") && !is_shard_continuation(&lower)
}

/// `<name>-00002-of-00004` is a continuation shard; keep only `-00001-`.
fn is_shard_continuation(stem: &str) -> bool {
    let mut parts = stem.rsplitn(3, '-');
    let total = parts.next().unwrap_or_default();
    let separator = parts.next().unwrap_or_default();
    let head = parts.next().unwrap_or_default();
    if separator != "of" || !total.chars().all(|c| c.is_ascii_digit()) || head.is_empty() {
        return false;
    }
    let index = head.rsplit('-').next().unwrap_or_default();
    index.len() >= 5
        && index.chars().all(|c| c.is_ascii_digit())
        && index.parse::<u32>().is_ok_and(|n| n > 1)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn filters_mmproj_and_shard_continuations() {
        assert!(is_launchable_model(Path::new("C:/m/Qwen-7B.gguf")));
        assert!(is_launchable_model(Path::new("C:/m/Qwen-7B-MERGED.GGUF")));
        assert!(!is_launchable_model(Path::new(
            "C:/m/mmproj-Qwen-7B-f16.gguf"
        )));
        assert!(!is_launchable_model(Path::new(
            "C:/m/Qwen-7B-mmproj-f16.gguf"
        )));
        assert!(is_launchable_model(Path::new(
            "C:/m/Qwen-7B-00001-of-00004.gguf"
        )));
        assert!(!is_launchable_model(Path::new(
            "C:/m/Qwen-7B-00002-of-00004.gguf"
        )));
        assert!(!is_launchable_model(Path::new("C:/m/notes.txt")));
    }

    #[test]
    fn walk_keeps_only_launchable_models() {
        let dir = tempfile::TempDir::new().unwrap();
        for name in [
            "a.gguf",
            "a-00001-of-00002.gguf",
            "a-00002-of-00002.gguf",
            "mmproj-a-f16.gguf",
            "readme.md",
        ] {
            std::fs::write(dir.path().join(name), b"x").unwrap();
        }
        let mut found = Vec::new();
        walk_models_recursive(dir.path(), &mut found);
        let mut names: Vec<String> = found
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, vec!["a-00001-of-00002.gguf", "a.gguf"]);
    }
}
