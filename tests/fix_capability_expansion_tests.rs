use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[path = "evolution_test_support.rs"]
mod evolution_test_support;

use eva_runtime_with_task_validator::{
    print_doctor, run_doctor, run_fix, FixOnly, FixRequest, FixRiskCap, FixStatus,
};

#[test]
fn fix_missing_gitignore_target_dry_run_is_read_only() {
    let root = rust_repo("fix-gitignore-dry-run", true, true);
    fs::write(root.join(".gitignore"), "*.log\n").expect("gitignore");
    let before = fs::read_to_string(root.join(".gitignore")).expect("gitignore before");
    let report = run_fix(request(
        &root,
        false,
        Some(FixOnly::Hygiene),
        FixRiskCap::Low,
    ))
    .expect("fix report");
    let after = fs::read_to_string(root.join(".gitignore")).expect("gitignore after");
    assert_eq!(before, after);
    assert_eq!(
        report.detected_problem.as_deref(),
        Some("missing_gitignore_target")
    );
    assert!(!report.source_mutation);
    assert!(matches!(report.status, FixStatus::ProposalCreated));
    evolution_test_support::remove_root(&root);
}

#[test]
fn fix_missing_gitignore_target_apply_adds_target_entry() {
    let root = rust_repo("fix-gitignore-apply", true, true);
    fs::write(root.join(".gitignore"), "*.log\n").expect("gitignore");
    let report = run_fix(request_apply(
        &root,
        Some(FixOnly::Hygiene),
        FixRiskCap::Low,
    ))
    .expect("fix report");
    let contents = fs::read_to_string(root.join(".gitignore")).expect("gitignore");
    assert!(contents.contains("*.log"));
    assert!(contents.contains("target/"));
    assert_eq!(contents.matches("target/").count(), 1);
    assert!(report.source_mutation);
    evolution_test_support::remove_root(&root);
}

#[test]
fn fix_missing_clippy_ci_updates_only_rust_ci_workflow() {
    let root = rust_repo("fix-clippy-ci", false, true);
    fs::create_dir_all(root.join(".github/workflows")).expect("workflow dir");
    fs::write(
        root.join(".github/workflows/rust-ci.yml"),
        "name: Rust CI\non: [push]\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - run: cargo fmt --check\n      - run: cargo check --all-targets\n      - run: cargo test\n",
    )
    .expect("workflow");
    let report =
        run_fix(request_apply(&root, Some(FixOnly::Ci), FixRiskCap::Low)).expect("fix report");
    let workflow =
        fs::read_to_string(root.join(".github/workflows/rust-ci.yml")).expect("workflow");
    assert!(workflow.contains("cargo clippy --all-targets -- -D warnings"));
    assert_eq!(
        report.detected_problem.as_deref(),
        Some("missing_clippy_ci")
    );
    assert_eq!(
        report.files_changed_by_patch,
        vec![".github/workflows/rust-ci.yml".to_string()]
    );
    assert!(report.source_mutation);
    evolution_test_support::remove_root(&root);
}

#[test]
fn fix_missing_clippy_ci_does_not_allow_arbitrary_workflow() {
    let root = rust_repo("fix-clippy-arbitrary", false, true);
    fs::create_dir_all(root.join(".github/workflows")).expect("workflow dir");
    fs::write(
        root.join(".github/workflows/rust-ci.yml"),
        "name: Rust CI\non: [push]\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - run: cargo fmt --check\n      - run: cargo check --all-targets\n      - run: cargo test\n",
    )
    .expect("workflow");
    fs::write(
        root.join(".github/workflows/release.yml"),
        "name: release\n",
    )
    .expect("release workflow");
    let report =
        run_fix(request_apply(&root, Some(FixOnly::Ci), FixRiskCap::Low)).expect("fix report");
    assert!(root.join(".github/workflows/release.yml").exists());
    let release = fs::read_to_string(root.join(".github/workflows/release.yml")).expect("release");
    assert_eq!(release, "name: release\n");
    assert_eq!(
        report.detected_problem.as_deref(),
        Some("missing_clippy_ci")
    );
    evolution_test_support::remove_root(&root);
}

#[test]
fn fix_missing_readme_usage_section_updates_readme() {
    let root = rust_repo("fix-readme-usage", true, true);
    fs::write(
        root.join("README.md"),
        "# Fixture\n\n## Validation\n\n```bash\ncargo fmt --check\ncargo check\ncargo test\n```\n",
    )
    .expect("readme");
    let report =
        run_fix(request_apply(&root, Some(FixOnly::Docs), FixRiskCap::Low)).expect("fix report");
    let contents = fs::read_to_string(root.join("README.md")).expect("readme");
    assert!(contents.contains("## Usage"));
    assert!(contents.contains("cargo run -- repair-bench"));
    assert_eq!(
        report.detected_problem.as_deref(),
        Some("missing_readme_usage_section")
    );
    evolution_test_support::remove_root(&root);
}

#[test]
fn fix_readme_usage_does_not_duplicate_existing_section() {
    let root = rust_repo("fix-readme-no-dup", true, true);
    fs::write(
        root.join("README.md"),
        "# Fixture\n\n## Validation\n\n```bash\ncargo fmt --check\ncargo check\ncargo test\n```\n\n## Usage\n\n```bash\ncargo run -- doctor .\n```\n",
    )
    .expect("readme");
    let report =
        run_fix(request(&root, false, Some(FixOnly::Docs), FixRiskCap::Low)).expect("fix report");
    assert_eq!(report.detected_problem, None);
    let contents = fs::read_to_string(root.join("README.md")).expect("readme");
    assert_eq!(contents.matches("## Usage").count(), 1);
    evolution_test_support::remove_root(&root);
}

#[test]
fn doctor_reports_new_hygiene_findings() {
    let root = rust_repo("doctor-new-hygiene", false, true);
    fs::create_dir_all(root.join(".github/workflows")).expect("workflow dir");
    fs::write(
        root.join(".github/workflows/rust-ci.yml"),
        "name: Rust CI\non: [push]\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - run: cargo fmt --check\n      - run: cargo check --all-targets\n      - run: cargo test\n",
    )
    .expect("workflow");
    fs::write(
        root.join("README.md"),
        "# Fixture\n\n## Validation\n\n```bash\ncargo fmt --check\ncargo check\ncargo test\n```\n",
    )
    .expect("readme");
    fs::write(root.join(".gitignore"), "*.log\n").expect("gitignore");
    let report = run_doctor(doctor_request(&root)).expect("doctor report");
    assert!(report
        .findings
        .iter()
        .any(|finding| finding.code == "rust_ci_clippy_missing"));
    assert!(report
        .findings
        .iter()
        .any(|finding| finding.code == "readme_usage_missing"));
    assert!(report
        .findings
        .iter()
        .any(|finding| finding.code == "gitignore_target_missing"));
    let output = print_doctor(doctor_request(&root)).expect("doctor text");
    assert!(output.contains("README usage section missing"));
    evolution_test_support::remove_root(&root);
}

#[test]
fn repair_bench_phase24x_runs_new_cases() {
    let root = evolution_test_support::unique_evolution_root("repair-bench-phase24x");
    let output_dir = root.join("bench-output");
    let report = eva_runtime_with_task_validator::run_repair_bench(
        eva_runtime_with_task_validator::RepairBenchRequest {
            bench_id: "repair-bench-test-phase24x".to_string(),
            suite: "phase24x".to_string(),
            output_dir: output_dir.clone(),
            no_llm: true,
            json: false,
        },
    )
    .expect("bench report");
    assert_eq!(report.total_cases, 8);
    assert_eq!(report.passed_cases, 7);
    assert_eq!(report.partial_cases, 1);
    assert_eq!(report.failed_cases, 0);
    assert_eq!(report.metrics.actionable_cases, 7);
    for case_id in [
        "missing_gitignore_target",
        "missing_clippy_ci",
        "missing_readme_usage_section",
    ] {
        assert!(report
            .case_results
            .iter()
            .any(|case| case.case_id == case_id));
    }
    evolution_test_support::remove_root(&root);
}

fn request(root: &PathBuf, json: bool, only: Option<FixOnly>, risk_cap: FixRiskCap) -> FixRequest {
    FixRequest {
        fix_id: unique_fix_id("request"),
        target_path: root.clone(),
        dry_run: !json,
        apply: false,
        only,
        max_files: 3,
        risk_cap,
        no_llm: true,
        provider: Some("rule_based".to_string()),
        evidence_dir: PathBuf::from(".eva/fix"),
    }
}

fn request_apply(root: &PathBuf, only: Option<FixOnly>, risk_cap: FixRiskCap) -> FixRequest {
    FixRequest {
        fix_id: unique_fix_id("apply"),
        target_path: root.clone(),
        dry_run: false,
        apply: true,
        only,
        max_files: 3,
        risk_cap,
        no_llm: true,
        provider: Some("rule_based".to_string()),
        evidence_dir: PathBuf::from(".eva/fix"),
    }
}

fn doctor_request(root: &PathBuf) -> eva_runtime_with_task_validator::DoctorRequest {
    eva_runtime_with_task_validator::DoctorRequest {
        doctor_id: unique_fix_id("doctor"),
        target_path: root.clone(),
        validate: false,
        json: false,
        no_llm: true,
        evidence_dir: PathBuf::from(".eva/doctor"),
    }
}

fn rust_repo(name: &str, readme_validation: bool, git_clean: bool) -> PathBuf {
    let root = evolution_test_support::unique_evolution_root(name);
    fs::create_dir_all(root.join("src")).expect("src");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname=\"fix_expansion_fixture\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
    )
    .expect("cargo");
    fs::write(root.join("src/lib.rs"), "pub fn probe() -> bool { true }\n").expect("lib");
    if readme_validation {
        fs::write(
            root.join("README.md"),
            "# Fixture\n\n## Validation\n\n```bash\ncargo fmt --check\ncargo check\ncargo test\n```\n",
        )
        .expect("readme");
    } else {
        fs::write(root.join("README.md"), "# Fixture\n").expect("readme");
    }
    if git_clean {
        let _ = Command::new("git").arg("init").current_dir(&root).output();
    }
    root
}

fn unique_fix_id(name: &str) -> String {
    format!("fix-expansion-{}-{name}", std::process::id())
}
