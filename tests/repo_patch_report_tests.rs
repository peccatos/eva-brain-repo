#[path = "tool_test_support.rs"]
mod tool_test_support;

use eva_runtime_with_task_validator::{
    run_repo_patch_report, RepoPatchCliConfig, RepoPatchMachineSummary, RepoPatchStatus,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tool_test_support::unique_tool_root;

fn init_git_repo(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let root = unique_tool_root(name);
    fs::create_dir_all(&root).expect("root dir");
    for (rel, contents) in files {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent dir");
        }
        fs::write(path, contents).expect("file");
    }
    run_git(&root, &["init", "-q"]);
    run_git(&root, &["config", "user.email", "eva@example.com"]);
    run_git(&root, &["config", "user.name", "Eva"]);
    run_git(&root, &["add", "."]);
    run_git(&root, &["commit", "-q", "-m", "initial"]);
    root
}

fn run_git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(root)
        .args(args)
        .status()
        .expect("git command");
    assert!(status.success(), "git {:?} failed", args);
}

fn load_summary(path: &Path) -> RepoPatchMachineSummary {
    let contents = fs::read_to_string(path).expect("summary exists");
    serde_json::from_str(&contents).expect("summary json")
}

#[test]
fn repo_patch_report_writes_report_summary_and_sections() {
    let source_repo = init_git_repo(
        "repo_patch_report_ok",
        &[
            (
                "Cargo.toml",
                r#"[package]
name = "sample_repo"
version = "0.1.0"
edition = "2021"
"#,
            ),
            ("src/lib.rs", "pub fn meaning() -> i32 { 42 }\n"),
        ],
    );
    let output_root = unique_tool_root("repo_patch_report_output");
    let report_path = output_root.join("eva_output").join("report.md");
    let summary_path = output_root.join("eva_output").join("summary.json");
    let config = RepoPatchCliConfig {
        repo_url: source_repo.display().to_string(),
        branch: None,
        max_changed_files: 10,
        report_path: report_path.display().to_string(),
        machine_summary_path: summary_path.display().to_string(),
    };

    let execution = run_repo_patch_report(&config).expect("repo patch execution");
    let report = fs::read_to_string(&report_path).expect("report exists");
    let summary = load_summary(&summary_path);

    assert_eq!(execution.status, RepoPatchStatus::Ok);
    assert_eq!(summary.status, RepoPatchStatus::Ok);
    assert!(!summary.changed_files.is_empty());
    assert!(report.contains("# EVA Report"));
    assert!(report.contains("## Repo"));
    assert!(report.contains("## Summary"));
    assert!(report.contains("## Changed files"));

    for changed in &summary.changed_files {
        let section = format!("## {}", changed.path);
        assert!(report.contains(&section), "missing section {section}");
    }

    assert!(report.contains("```yaml"));
    assert!(report.contains("```rust"));
    assert!(execution.stdout_output().contains("[repo]"));
    assert!(execution.stdout_output().contains("[report]"));
    assert!(execution.stdout_output().contains("[changed_files]"));
    assert!(execution.stdout_output().contains("[status]"));
}

#[test]
fn repo_patch_report_fails_cleanly_for_non_rust_repo() {
    let source_repo = init_git_repo(
        "repo_patch_report_fail",
        &[
            ("README.md", "# sample\n"),
            ("src/index.txt", "plain text\n"),
        ],
    );
    let output_root = unique_tool_root("repo_patch_report_fail_output");
    let report_path = output_root.join("eva_output").join("report.md");
    let summary_path = output_root.join("eva_output").join("summary.json");
    let config = RepoPatchCliConfig {
        repo_url: source_repo.display().to_string(),
        branch: None,
        max_changed_files: 10,
        report_path: report_path.display().to_string(),
        machine_summary_path: summary_path.display().to_string(),
    };

    let execution = run_repo_patch_report(&config).expect("repo patch execution");
    let report = fs::read_to_string(&report_path).expect("report exists");
    let summary = load_summary(&summary_path);

    assert_eq!(execution.status, RepoPatchStatus::Fail);
    assert_eq!(summary.status, RepoPatchStatus::Fail);
    assert!(summary.changed_files.is_empty());
    assert!(report.contains("# EVA Report"));
    assert!(report.contains("## Repo"));
    assert!(report.contains("## Summary"));
    assert!(report.contains("## Changed files"));
    assert!(execution.stdout_output().contains(&config.report_path));
}

#[test]
fn repo_patch_cli_parser_accepts_contract_flags() {
    let config = RepoPatchCliConfig::parse_from_iter([
        "--repo",
        "https://github.com/example/project",
        "--branch",
        "main",
        "--max-changed-files",
        "7",
        "--report-path",
        "./eva_output/custom_report.md",
        "--machine-summary-path",
        "./eva_output/custom_summary.json",
    ])
    .expect("valid cli");

    assert_eq!(config.repo_url, "https://github.com/example/project");
    assert_eq!(config.branch.as_deref(), Some("main"));
    assert_eq!(config.max_changed_files, 7);
    assert_eq!(config.report_path, "./eva_output/custom_report.md");
    assert_eq!(
        config.machine_summary_path,
        "./eva_output/custom_summary.json"
    );
}
