#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn unique_evolution_root(name: &str) -> PathBuf {
    let sanitized = sanitize_name(name);
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(".eva-evolution-tests")
        .join(format!("eva-evolution-{sanitized}-{pid}-{nanos}-{counter}"));
    fs::create_dir_all(&root).expect("create root");
    root
}

pub fn remove_root(root: &Path) {
    let _ = fs::remove_dir_all(root);
}

pub fn eva_command(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_eva_runtime_with_task_validator"));
    let tmp = root.join(".tmp");
    fs::create_dir_all(&tmp).expect("create tmp");
    command.current_dir(root);
    command.env("CARGO_TARGET_DIR", root.join("target"));
    command.env("TMPDIR", tmp);
    command
}

fn sanitize_name(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let trimmed = sanitized.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "test".to_string()
    } else {
        trimmed
    }
}
