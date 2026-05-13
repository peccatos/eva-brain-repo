use std::path::Path;
use std::process::Command;

use crate::agent::storage::{id, memory_path, now_unix, save_json_pretty};
use crate::contracts::WorkspaceInspection;

pub fn inspect_workspace(
    project_root: &str,
    memory_root: &str,
) -> Result<WorkspaceInspection, String> {
    let root = Path::new(project_root);
    let cargo_toml_exists = root.join("Cargo.toml").exists();
    let inspection = WorkspaceInspection {
        inspection_id: id("inspection"),
        generated_at: now_unix(),
        repo_root: root.display().to_string(),
        git_status: git_stdout(root, &["status", "--short"]).unwrap_or_else(|| "unknown".into()),
        branch: git_stdout(root, &["branch", "--show-current"]),
        head: git_stdout(root, &["rev-parse", "HEAD"]),
        language: if cargo_toml_exists {
            "rust".into()
        } else {
            "unknown".into()
        },
        cargo_project: cargo_toml_exists,
        cargo_toml_exists,
        lockfile_exists: root.join("Cargo.lock").exists(),
        entrypoints: ["src/main.rs", "src/lib.rs"]
            .into_iter()
            .filter(|p| root.join(p).exists())
            .map(str::to_string)
            .collect(),
        source_dirs: if root.join("src").is_dir() {
            vec!["src/".into()]
        } else {
            Vec::new()
        },
        test_dirs: if root.join("tests").is_dir() {
            vec!["tests/".into()]
        } else {
            Vec::new()
        },
        docs_dirs: if root.join("docs").is_dir() {
            vec!["docs/".into()]
        } else {
            Vec::new()
        },
        available_commands: vec![
            "cargo fmt --check".into(),
            "cargo check".into(),
            "cargo test".into(),
        ],
        risk_zones: vec!["src/".into(), "Cargo.toml".into()],
        ignored_zones: vec![
            "memory/".into(),
            "target/".into(),
            ".git/".into(),
            "sandboxes/".into(),
        ],
        warnings: Vec::new(),
        blockers: Vec::new(),
    };
    save_json_pretty(
        &memory_path(
            memory_root,
            &["inspections", &format!("{}.json", inspection.inspection_id)],
        ),
        &inspection,
    )?;
    save_json_pretty(
        &memory_path(memory_root, &["inspections", "latest_inspection.json"]),
        &inspection,
    )?;
    Ok(inspection)
}

pub fn print_workspace_inspection(project_root: &str, memory_root: &str) -> Result<String, String> {
    let inspection = inspect_workspace(project_root, memory_root)?;
    Ok(format!(
        "EVA Workspace Inspection\nlanguage={}\ncargo_project={}\ngit_status={}\nbranch={}\nhead={}\nentrypoints={}\nrisk_zones={}",
        inspection.language,
        inspection.cargo_project,
        if inspection.git_status.trim().is_empty() { "clean" } else { "dirty" },
        inspection.branch.as_deref().unwrap_or("unknown"),
        inspection.head.as_deref().unwrap_or("unknown"),
        inspection.entrypoints.join(","),
        inspection.risk_zones.join(",")
    ))
}

fn git_stdout(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
