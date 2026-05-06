use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::sandbox::limits::DEFAULT_SANDBOX_ROOT;

static SANDBOX_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn create_sandbox_path() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let counter = SANDBOX_COUNTER.fetch_add(1, Ordering::Relaxed);
    Path::new(DEFAULT_SANDBOX_ROOT)
        .join(format!(
            "eva-sandbox-{now}-{}-{counter}",
            std::process::id()
        ))
        .to_string_lossy()
        .to_string()
}

pub fn destroy_sandbox(path: &str) -> Result<(), String> {
    let path = Path::new(path);
    if !path.exists() {
        return Ok(());
    }
    ensure_sandbox_path(path)?;
    fs::remove_dir_all(path).map_err(|error| format!("failed to destroy sandbox: {error}"))
}

fn ensure_sandbox_path(path: &Path) -> Result<(), String> {
    let normalized = normalize(path);
    if !normalized
        .components()
        .any(|component| component.as_os_str() == DEFAULT_SANDBOX_ROOT)
    {
        return Err("refusing to destroy path outside sandboxes/".to_string());
    }
    Ok(())
}

fn normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        normalized.push(component.as_os_str());
    }
    normalized
}
