use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::path::Path;

pub fn ensure_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|error| format!("failed to create {}: {error}", path.display()))
}

pub fn save_json_pretty<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }
    let contents =
        serde_json::to_string_pretty(value).map_err(|error| format!("serialize json: {error}"))?;
    fs::write(path, contents)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

pub fn load_json<T: DeserializeOwned>(path: &Path) -> Result<T, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

pub fn now_unix() -> u64 {
    crate::evolution::memory::now_unix()
}

pub fn id(prefix: &str) -> String {
    format!("{prefix}-{}-{}", now_unix(), std::process::id())
}

pub fn memory_path(root: &str, parts: &[&str]) -> std::path::PathBuf {
    let mut path = Path::new(root).to_path_buf();
    for part in parts {
        path.push(part);
    }
    path
}
