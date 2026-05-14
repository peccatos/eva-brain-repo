use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[path = "evolution_test_support.rs"]
mod evolution_test_support;

use eva_runtime_with_task_validator::{
    print_external_trial, run_external_trial, ExternalTrialRequest,
};

#[test]
fn external_trial_blocks_missing_repo_path() {
    let root = evolution_test_support::unique_evolution_root("external-trial-missing");
    let repo = root.join("missing-repo");
    let report = run_external_trial(trial_request(vec![repo.clone()], &root)).expect("trial");
    assert_eq!(report.repos_total, 1);
    assert_eq!(report.repos_skipped, 1);
    assert_eq!(report.repo_results[0].status, "skipped");
    assert!(report.repo_results[0]
        .notes
        .iter()
        .any(|note| note == "blocker:target_path_does_not_exist"));
    evolution_test_support::remove_root(&root);
}

#[test]
fn external_trial_blocks_file_target() {
    let root = evolution_test_support::unique_evolution_root("external-trial-file");
    let repo = root.join("not-a-dir.txt");
    fs::write(&repo, "content").expect("file");
    let report = run_external_trial(trial_request(vec![repo.clone()], &root)).expect("trial");
    assert_eq!(report.repo_results[0].status, "skipped");
    assert!(report.repo_results[0]
        .notes
        .iter()
        .any(|note| note == "blocker:target_path_not_directory"));
    evolution_test_support::remove_root(&root);
}

#[test]
fn external_trial_runs_doctor_on_temp_repo() {
    let root = rust_repo("external-trial-doctor");
    let trial_root = trial_output_root("external-trial-doctor-output");
    let report = run_external_trial(trial_request(vec![root.clone()], &trial_root)).expect("trial");
    let repo = &report.repo_results[0];
    assert_eq!(repo.status, "processed");
    assert!(!repo.doctor_findings.is_empty());
    assert!(!repo.suggested_fix_commands.is_empty());
    evolution_test_support::remove_root(&root);
    evolution_test_support::remove_root(&trial_root);
}

#[test]
fn external_trial_runs_fix_dry_run_only() {
    let root = rust_repo("external-trial-fix-dry-run");
    fs::write(root.join("README.md"), "# Fixture\n").expect("readme");
    let trial_root = trial_output_root("external-trial-fix-dry-run-output");
    let report = run_external_trial(trial_request(vec![root.clone()], &trial_root)).expect("trial");
    let repo = &report.repo_results[0];
    assert!(repo
        .dry_run_fix_reports
        .iter()
        .all(|fix| !fix.source_mutation));
    assert!(repo
        .dry_run_fix_reports
        .iter()
        .all(|fix| fix.evidence_written));
    evolution_test_support::remove_root(&root);
    evolution_test_support::remove_root(&trial_root);
}

#[test]
fn external_trial_does_not_mutate_external_repo_source() {
    let root = rust_repo("external-trial-clean");
    let trial_root = trial_output_root("external-trial-clean-output");
    let before = git_status_short(&root);
    let report = run_external_trial(trial_request(vec![root.clone()], &trial_root)).expect("trial");
    let after = git_status_short(&root);
    assert_eq!(before, after);
    assert!(!root.join(".eva").exists());
    assert!(report.repo_results[0].evidence_dir.starts_with(&trial_root));
    evolution_test_support::remove_root(&root);
    evolution_test_support::remove_root(&trial_root);
}

#[test]
fn external_trial_writes_report_json_and_markdown() {
    let root = rust_repo("external-trial-report");
    let trial_root = trial_output_root("external-trial-report-output");
    let report = run_external_trial(trial_request(vec![root.clone()], &trial_root)).expect("trial");
    assert!(report.output_dir.join("report.json").exists());
    assert!(report.output_dir.join("report.md").exists());
    let rendered =
        print_external_trial(trial_request(vec![root.clone()], &trial_root)).expect("print trial");
    assert!(rendered.contains("EVE External Trial Report"));
    assert!(report.output_dir.exists());
    evolution_test_support::remove_root(&root);
    evolution_test_support::remove_root(&trial_root);
}

#[test]
fn external_trial_does_not_require_network_or_openai() {
    let root = rust_repo("external-trial-offline");
    let trial_root = trial_output_root("external-trial-offline-output");
    let report = run_external_trial(trial_request(vec![root.clone()], &trial_root)).expect("trial");
    assert_eq!(report.repos_processed, 1);
    assert!(report.blockers.is_empty());
    evolution_test_support::remove_root(&root);
    evolution_test_support::remove_root(&trial_root);
}

fn trial_request(repo_paths: Vec<PathBuf>, output_root: &PathBuf) -> ExternalTrialRequest {
    let trial_id = format!("external-trial-test-{}", std::process::id());
    ExternalTrialRequest {
        trial_id: trial_id.clone(),
        repo_paths,
        output_dir: output_root.join(&trial_id),
        json: false,
    }
}

fn rust_repo(name: &str) -> PathBuf {
    let root = evolution_test_support::unique_evolution_root(name);
    fs::create_dir_all(root.join("src")).expect("src");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname=\"external_trial_fixture\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
    )
    .expect("cargo");
    fs::write(root.join("src/lib.rs"), "pub fn probe() -> bool { true }\n").expect("lib");
    fs::write(root.join("README.md"), "# Fixture\n").expect("readme");
    fs::create_dir_all(root.join(".github/workflows")).expect("workflow");
    fs::write(
        root.join(".github/workflows/rust-ci.yml"),
        "name: Rust CI\non: [push]\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - run: cargo fmt --check\n      - run: cargo check --all-targets\n      - run: cargo test\n",
    )
    .expect("workflow");
    let _ = Command::new("git").arg("init").current_dir(&root).output();
    root
}

fn trial_output_root(name: &str) -> PathBuf {
    let root = evolution_test_support::unique_evolution_root(name);
    fs::create_dir_all(&root).expect("trial root");
    root
}

fn git_status_short(root: &PathBuf) -> Vec<String> {
    let output = Command::new("git")
        .args(["status", "--short"])
        .current_dir(root)
        .output()
        .expect("git status");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}
