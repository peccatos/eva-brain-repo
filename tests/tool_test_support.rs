#![allow(dead_code)]
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TOOL_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn unique_tool_root(name: &str) -> PathBuf {
    let sanitized = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let pid = std::process::id();
    let counter = TOOL_COUNTER.fetch_add(1, Ordering::Relaxed);
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".eva-runtime-tests")
        .join(format!(
            "eva-tool-{}-{pid}-{stamp}-{counter}",
            if sanitized.is_empty() {
                "test"
            } else {
                &sanitized
            }
        ))
}

pub fn init_tool_workspace(name: &str) -> PathBuf {
    let root = unique_tool_root(name);
    fs::create_dir_all(&root).expect("root dir");
    fs::write(
        root.join("tool_registry.json"),
        include_str!("../tool_registry.json"),
    )
    .expect("registry");
    fs::write(
        root.join("tool_policy.json"),
        include_str!("../tool_policy.json"),
    )
    .expect("policy");
    root
}

pub fn init_cargo_tool_workspace(name: &str, lib_contents: &str) -> PathBuf {
    let root = init_tool_workspace(name);
    fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "eva_tool_test_workspace"
version = "0.1.0"
edition = "2021"

[lib]
doctest = false

[dependencies]
"#,
    )
    .expect("cargo toml");
    let src = root.join("src");
    fs::create_dir_all(&src).expect("src dir");
    fs::write(src.join("lib.rs"), lib_contents).expect("lib");
    root
}

pub fn init_git_tool_workspace(name: &str, lib_contents: &str) -> PathBuf {
    let root = init_cargo_tool_workspace(name, lib_contents);
    run_git(&root, &["init", "-q"]);
    run_git(&root, &["config", "user.email", "eva@example.com"]);
    run_git(&root, &["config", "user.name", "Eva"]);
    run_git(&root, &["add", "."]);
    run_git(&root, &["commit", "-q", "-m", "initial"]);
    root
}

#[allow(dead_code)]
pub fn write_rel(root: &PathBuf, rel: &str, contents: &str) {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent");
    }
    fs::write(path, contents).expect("write file");
}

fn run_git(root: &PathBuf, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(root)
        .args(args)
        .status()
        .expect("git command");
    assert!(status.success(), "git {:?} failed", args);
}
