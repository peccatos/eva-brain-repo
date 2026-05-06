#![allow(dead_code)]
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn unique_tool_root(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let pid = std::process::id();
    path.push(format!("eva_tool_layer_{name}_{pid}_{stamp}"));
    path
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
