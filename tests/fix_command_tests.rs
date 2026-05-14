use std::fs;
use std::path::PathBuf;

#[path = "evolution_test_support.rs"]
mod evolution_test_support;

use eva_runtime_with_task_validator::{
    print_fix, run_fix, FixOnly, FixRequest, FixRiskCap, FixStatus,
};

#[test]
fn dry_run_does_not_mutate_missing_ci_project() {
    let root = rust_fixture("fix-dry-run-ci", true);
    let report = run_fix(request(&root, false, None, 3, FixRiskCap::Low)).expect("fix report");
    assert!(matches!(report.status, FixStatus::ProposalCreated));
    assert!(report.evidence_dir.join("request.json").exists());
    assert!(report.evidence_dir.join("report.md").exists());
    assert!(!report.source_mutation);
    assert!(report.evidence_written);
    assert!(!root.join(".github/workflows/rust-ci.yml").exists());
    evolution_test_support::remove_root(&root);
}

#[test]
fn fix_blocks_missing_target_path() {
    let root = evolution_test_support::unique_evolution_root("fix-missing-target");
    let target = root.join("does-not-exist").join("repo");
    let report = run_fix(FixRequest {
        fix_id: unique_fix_id("missing-target"),
        target_path: target.clone(),
        dry_run: false,
        apply: true,
        only: Some(FixOnly::Ci),
        max_files: 3,
        risk_cap: FixRiskCap::Low,
        no_llm: true,
        provider: Some("rule_based".to_string()),
        evidence_dir: PathBuf::from(".eva/fix"),
    })
    .expect("blocked fix report");
    assert_eq!(report.status, FixStatus::Blocked);
    assert!(report
        .blockers
        .iter()
        .any(|blocker| blocker == "target_path_does_not_exist"));
    assert!(!report.source_mutation);
    assert!(!report.evidence_written);
    assert!(report.evidence_dir.as_os_str().is_empty());
    assert!(!target.exists());
    assert!(!root.join("does-not-exist").exists());
    assert!(!root.join(".eva").exists());
    evolution_test_support::remove_root(&root);
}

#[test]
fn fix_blocks_file_target_path() {
    let root = evolution_test_support::unique_evolution_root("fix-file-target");
    let file_target = root.join("not_a_repo.txt");
    fs::write(&file_target, "content").expect("file");
    let report = run_fix(FixRequest {
        fix_id: unique_fix_id("file-target"),
        target_path: file_target.clone(),
        dry_run: false,
        apply: true,
        only: Some(FixOnly::Ci),
        max_files: 3,
        risk_cap: FixRiskCap::Low,
        no_llm: true,
        provider: Some("rule_based".to_string()),
        evidence_dir: PathBuf::from(".eva/fix"),
    })
    .expect("blocked fix report");
    assert_eq!(report.status, FixStatus::Blocked);
    assert!(report
        .blockers
        .iter()
        .any(|blocker| blocker == "target_path_not_directory"));
    assert!(!report.source_mutation);
    assert!(!report.evidence_written);
    assert!(report.evidence_dir.as_os_str().is_empty());
    assert!(file_target.exists());
    assert!(!root.join(".eva").exists());
    evolution_test_support::remove_root(&root);
}

#[test]
fn fix_invalid_target_does_not_select_openai() {
    let root = evolution_test_support::unique_evolution_root("fix-invalid-openai");
    let target = root.join("missing").join("repo");
    let report = run_fix(FixRequest {
        fix_id: unique_fix_id("invalid-openai"),
        target_path: target,
        dry_run: false,
        apply: true,
        only: Some(FixOnly::Ci),
        max_files: 3,
        risk_cap: FixRiskCap::Low,
        no_llm: false,
        provider: Some("openai".to_string()),
        evidence_dir: PathBuf::from(".eva/fix"),
    })
    .expect("blocked fix report");
    assert_eq!(report.status, FixStatus::Blocked);
    assert!(!report.llm_used);
    assert_ne!(report.provider, "openai");
    assert!(report.warnings.is_empty());
    evolution_test_support::remove_root(&root);
}

#[test]
fn apply_missing_ci_creates_workflow_and_evidence() {
    let root = rust_fixture("fix-apply-ci", true);
    let report =
        run_fix(request(&root, true, Some(FixOnly::Ci), 3, FixRiskCap::Low)).expect("fix report");
    assert!(root.join(".github/workflows/rust-ci.yml").exists());
    assert!(matches!(
        report.status,
        FixStatus::Applied | FixStatus::ValidationPassed | FixStatus::ValidationFailed
    ));
    assert!(report.source_mutation);
    assert!(report.evidence_written);
    assert!(report
        .validation_side_effects
        .iter()
        .any(|path| path == "Cargo.lock"));
    assert!(report
        .files_changed_by_patch
        .iter()
        .all(|path| path != "Cargo.lock"));
    assert!(report.evidence_dir.join("apply_result.json").exists());
    evolution_test_support::remove_root(&root);
}

#[test]
fn apply_missing_smoke_test_creates_file_and_records_validation() {
    let root = rust_fixture("fix-apply-tests", true);
    let report = run_fix(request(
        &root,
        true,
        Some(FixOnly::Tests),
        3,
        FixRiskCap::Low,
    ))
    .expect("fix report");
    assert!(root.join("tests/eve_smoke.rs").exists());
    assert!(matches!(
        report.status,
        FixStatus::Applied | FixStatus::ValidationPassed | FixStatus::ValidationFailed
    ));
    assert!(report.source_mutation);
    assert!(report.evidence_written);
    assert!(report.evidence_dir.join("validation.json").exists());
    evolution_test_support::remove_root(&root);
}

#[test]
fn one_fix_only_respects_priority() {
    let root = rust_fixture("fix-priority", true);
    let report = run_fix(request(&root, false, None, 3, FixRiskCap::Low)).expect("fix report");
    assert_eq!(report.detected_problem.as_deref(), Some("missing_ci"));
    assert_eq!(
        report.files_planned,
        vec![".github/workflows/rust-ci.yml".to_string()]
    );
    assert!(!report
        .files_planned
        .iter()
        .any(|path| path == "tests/eve_smoke.rs"));
    evolution_test_support::remove_root(&root);
}

#[test]
fn risk_cap_and_max_files_are_respected() {
    let root = rust_fixture("fix-risk-cap", true);
    let report =
        run_fix(request(&root, false, Some(FixOnly::Ci), 1, FixRiskCap::Low)).expect("fix report");
    assert!(matches!(report.status, FixStatus::ProposalCreated));
    assert_eq!(report.risk, "low");
    assert_eq!(report.files_planned.len(), 1);
    evolution_test_support::remove_root(&root);
}

#[test]
fn unknown_project_reports_no_actionable_problem_without_panic() {
    let root = evolution_test_support::unique_evolution_root("fix-unknown-project");
    fs::create_dir_all(&root).expect("root");
    let output = print_fix(request(&root, false, None, 3, FixRiskCap::Low)).expect("print");
    assert!(output.contains("Status:"));
    assert!(output.contains("Source mutation: false"));
    assert!(output.contains("Evidence written: true"));
    let report_path = root.join(".eva/fix");
    assert!(report_path.exists());
    evolution_test_support::remove_root(&root);
}

#[test]
fn external_target_evidence_defaults_under_target_repo_only() {
    let root = rust_fixture("fix-external-evidence", true);
    let fix_id = unique_fix_id("external-evidence");
    let report = run_fix(FixRequest {
        fix_id: fix_id.clone(),
        target_path: root.clone(),
        dry_run: false,
        apply: true,
        only: Some(FixOnly::Ci),
        max_files: 3,
        risk_cap: FixRiskCap::Low,
        no_llm: true,
        provider: Some("rule_based".to_string()),
        evidence_dir: PathBuf::from(".eva/fix"),
    })
    .expect("fix report");
    let target_evidence = root.join(".eva/fix").join(&fix_id);
    assert_eq!(report.evidence_dir, target_evidence);
    assert!(target_evidence.exists());
    let repo_root_evidence = std::env::current_dir()
        .expect("cwd")
        .join(".eva/fix")
        .join(&fix_id);
    assert!(!repo_root_evidence.exists());
    evolution_test_support::remove_root(&root);
}

fn request(
    root: &PathBuf,
    apply: bool,
    only: Option<FixOnly>,
    max_files: usize,
    risk_cap: FixRiskCap,
) -> FixRequest {
    FixRequest {
        fix_id: unique_fix_id("request"),
        target_path: root.clone(),
        dry_run: !apply,
        apply,
        only,
        max_files,
        risk_cap,
        no_llm: true,
        provider: Some("rule_based".to_string()),
        evidence_dir: PathBuf::from(".eva/fix"),
    }
}

fn rust_fixture(name: &str, readme: bool) -> PathBuf {
    let root = evolution_test_support::unique_evolution_root(name);
    fs::create_dir_all(root.join("src")).expect("src");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname=\"fix_fixture\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
    )
    .expect("cargo");
    fs::write(root.join("src/lib.rs"), "pub fn probe() -> bool { true }\n").expect("lib");
    if readme {
        fs::write(root.join("README.md"), "# Fixture\n").expect("readme");
    }
    root
}

fn unique_fix_id(name: &str) -> String {
    let sanitized = name.replace(|ch: char| !ch.is_ascii_alphanumeric(), "-");
    format!("fix-test-{}-{sanitized}", std::process::id())
}
