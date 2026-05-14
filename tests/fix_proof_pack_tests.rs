use std::fs;
use std::path::PathBuf;

#[path = "evolution_test_support.rs"]
mod evolution_test_support;

use eva_runtime_with_task_validator::{run_fix, FixOnly, FixRequest, FixRiskCap, FixStatus};

#[test]
fn proof_pack_missing_ci_writes_target_local_evidence_and_workflow() {
    let root = rust_fixture("proof-missing-ci", true);
    let report =
        run_fix(request(&root, true, Some(FixOnly::Ci), 3, FixRiskCap::Low)).expect("fix report");
    assert_eq!(report.detected_problem.as_deref(), Some("missing_ci"));
    assert!(matches!(
        report.status,
        FixStatus::Applied | FixStatus::ValidationPassed | FixStatus::ValidationFailed
    ));
    assert!(root.join(".github/workflows/rust-ci.yml").exists());
    assert!(!root.join(".github/workflows/deploy.yml").exists());
    assert!(!root.join(".github/workflows/release.yml").exists());
    assert!(report.evidence_dir.starts_with(root.join(".eva/fix")));
    assert!(report.evidence_dir.exists());
    assert!(report.evidence_written);
    assert!(report.source_mutation);
    evolution_test_support::remove_root(&root);
}

#[test]
fn proof_pack_missing_smoke_test_writes_target_local_evidence() {
    let root = rust_fixture("proof-missing-smoke", true);
    let report = run_fix(request(
        &root,
        true,
        Some(FixOnly::Tests),
        3,
        FixRiskCap::Low,
    ))
    .expect("fix report");
    assert_eq!(
        report.detected_problem.as_deref(),
        Some("missing_smoke_test")
    );
    assert!(root.join("tests/eve_smoke.rs").exists());
    assert!(report.evidence_dir.starts_with(root.join(".eva/fix")));
    assert!(report.evidence_dir.exists());
    assert!(report.evidence_written);
    evolution_test_support::remove_root(&root);
}

#[test]
fn proof_pack_readme_missing_validation_updates_readme() {
    let root = rust_fixture("proof-readme", false);
    let report = run_fix(request(
        &root,
        true,
        Some(FixOnly::Docs),
        3,
        FixRiskCap::Low,
    ))
    .expect("fix report");
    assert_eq!(
        report.detected_problem.as_deref(),
        Some("missing_readme_validation")
    );
    let readme = fs::read_to_string(root.join("README.md")).expect("readme");
    assert!(readme.contains("cargo fmt --check"));
    assert!(report.source_mutation);
    assert!(report.evidence_dir.starts_with(root.join(".eva/fix")));
    evolution_test_support::remove_root(&root);
}

#[test]
fn proof_pack_simple_missing_module_creates_stub() {
    let root = evolution_test_support::unique_evolution_root("proof-missing-module");
    fs::create_dir_all(root.join("src")).expect("src");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname=\"proof_missing_module\"\nversion=\"0.1.0\"\nedition=\"2021\"\n\n[lib]\npath=\"src/lib.rs\"\n",
    )
    .expect("cargo");
    fs::write(root.join("src/lib.rs"), "mod missing_module;\n").expect("lib");
    let report = run_fix(request(
        &root,
        true,
        Some(FixOnly::CargoCheck),
        3,
        FixRiskCap::Low,
    ))
    .expect("fix report");
    assert_eq!(
        report.detected_problem.as_deref(),
        Some("cargo_check_failure")
    );
    assert!(root.join("src/missing_module.rs").exists());
    assert!(report.evidence_dir.starts_with(root.join(".eva/fix")));
    evolution_test_support::remove_root(&root);
}

#[test]
fn proof_pack_unknown_empty_project_is_honest() {
    let root = evolution_test_support::unique_evolution_root("proof-empty-project");
    fs::create_dir_all(&root).expect("root");
    let report = run_fix(request(&root, true, None, 3, FixRiskCap::Low)).expect("fix report");
    assert_eq!(report.project_type, "unknown");
    assert!(matches!(
        report.status,
        FixStatus::NoActionableProblem | FixStatus::Blocked
    ));
    assert!(!report.source_mutation);
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
        fix_id: format!("proof-{}", std::process::id()),
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

fn rust_fixture(name: &str, with_readme: bool) -> PathBuf {
    let root = evolution_test_support::unique_evolution_root(name);
    fs::create_dir_all(root.join("src")).expect("src");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname=\"proof_fixture\"\nversion=\"0.1.0\"\nedition=\"2021\"\n\n[lib]\ndoctest=false\n",
    )
    .expect("cargo");
    fs::write(root.join("src/lib.rs"), "pub fn probe() -> bool { true }\n").expect("lib");
    if with_readme {
        fs::write(root.join("README.md"), "# Fixture\n").expect("readme");
    }
    root
}
