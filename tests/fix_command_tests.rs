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
    assert!(!root.join(".github/workflows/rust-ci.yml").exists());
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
    let report_path = root.join(".eva/fix");
    assert!(report_path.exists());
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
        fix_id: format!("fix-test-{}", std::process::id()),
        target_path: root.clone(),
        dry_run: !apply,
        apply,
        only,
        max_files,
        risk_cap,
        no_llm: true,
        provider: Some("rule_based".to_string()),
        evidence_dir: root.join(".eva/fix"),
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
