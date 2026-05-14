use std::fs;
use std::path::PathBuf;

#[path = "evolution_test_support.rs"]
mod evolution_test_support;

use eva_runtime_with_task_validator::{
    print_doctor, run_doctor, DoctorProjectType, DoctorRequest, DoctorStatus,
};

#[test]
fn doctor_blocks_missing_target_path() {
    let root = evolution_test_support::unique_evolution_root("doctor-missing-target");
    let target = root.join("missing").join("repo");
    let report = run_doctor(request(&target, false, false)).expect("doctor report");
    assert_eq!(report.status, DoctorStatus::Blocked);
    assert!(report
        .blockers
        .iter()
        .any(|blocker| blocker == "target_path_does_not_exist"));
    assert!(!report.source_mutation);
    assert!(!report.evidence_written);
    assert!(report.evidence_dir.is_none());
    assert!(!target.exists());
    assert!(!root.join("missing").exists());
    assert!(!root.join(".eva").exists());
    evolution_test_support::remove_root(&root);
}

#[test]
fn doctor_blocks_file_target_path() {
    let root = evolution_test_support::unique_evolution_root("doctor-file-target");
    let target = root.join("not_a_dir.txt");
    fs::write(&target, "hello").expect("file");
    let report = run_doctor(request(&target, false, false)).expect("doctor report");
    assert_eq!(report.status, DoctorStatus::Blocked);
    assert!(report
        .blockers
        .iter()
        .any(|blocker| blocker == "target_path_not_directory"));
    assert!(!report.source_mutation);
    assert!(!report.evidence_written);
    assert!(report.evidence_dir.is_none());
    assert!(target.exists());
    assert!(!root.join(".eva").exists());
    evolution_test_support::remove_root(&root);
}

#[test]
fn doctor_rust_project_reports_missing_ci() {
    let root = rust_fixture("doctor-missing-ci", false, true, true, false);
    let report = run_doctor(request(&root, false, false)).expect("doctor report");
    assert_eq!(report.project_type, DoctorProjectType::Rust);
    assert!(report
        .findings
        .iter()
        .any(|finding| finding.code == "rust_ci_missing"));
    assert!(report
        .suggestions
        .iter()
        .any(|suggestion| suggestion.command
            == format!("cargo run -- fix {} --only ci", root.display())));
    assert!(report.evidence_written);
    assert!(report.evidence_dir.is_some());
    evolution_test_support::remove_root(&root);
}

#[test]
fn doctor_rust_project_reports_missing_smoke_test() {
    let root = rust_fixture("doctor-missing-smoke", true, false, true, true);
    let report = run_doctor(request(&root, false, false)).expect("doctor report");
    assert!(report
        .findings
        .iter()
        .any(|finding| finding.code == "smoke_test_missing"));
    assert!(report
        .suggestions
        .iter()
        .any(|suggestion| suggestion.command
            == format!("cargo run -- fix {} --only tests", root.display())));
    evolution_test_support::remove_root(&root);
}

#[test]
fn doctor_rust_project_reports_readme_validation_missing() {
    let root = rust_fixture("doctor-missing-readme-validation", true, true, false, true);
    let report = run_doctor(request(&root, false, false)).expect("doctor report");
    assert!(report
        .findings
        .iter()
        .any(|finding| finding.code == "readme_validation_missing"));
    assert!(report
        .suggestions
        .iter()
        .any(|suggestion| suggestion.command
            == format!("cargo run -- fix {} --only docs", root.display())));
    evolution_test_support::remove_root(&root);
}

#[test]
fn doctor_suggests_fix_commands_without_apply() {
    let root = rust_fixture("doctor-suggestions", false, false, false, true);
    let report = run_doctor(request(&root, false, false)).expect("doctor report");
    assert!(!report.suggestions.is_empty());
    assert!(report
        .suggestions
        .iter()
        .all(|suggestion| !suggestion.command.contains("--apply")));
    evolution_test_support::remove_root(&root);
}

#[test]
fn doctor_writes_target_local_evidence() {
    let root = rust_fixture("doctor-evidence", true, true, true, true);
    let report = run_doctor(request(&root, false, false)).expect("doctor report");
    let expected = root.join(".eva/doctor").join(&report.doctor_id);
    assert_eq!(report.evidence_dir.as_ref(), Some(&expected));
    assert!(expected.exists());
    assert!(expected.join("request.json").exists());
    assert!(expected.join("report.json").exists());
    assert!(expected.join("report.md").exists());
    assert!(!std::env::current_dir()
        .expect("cwd")
        .join(".eva/doctor")
        .join(&report.doctor_id)
        .exists());
    evolution_test_support::remove_root(&root);
}

#[test]
fn doctor_json_output_is_parseable() {
    let root = rust_fixture("doctor-json", true, false, true, true);
    let mut request = request(&root, false, true);
    request.validate = false;
    let output = print_doctor(request).expect("doctor text");
    let report: eva_runtime_with_task_validator::DoctorReport =
        serde_json::from_str(&output).expect("json");
    assert_eq!(report.project_type, DoctorProjectType::Rust);
    assert!(report.evidence_written);
    evolution_test_support::remove_root(&root);
}

#[test]
fn doctor_default_does_not_create_cargo_lock() {
    let root = rust_fixture("doctor-no-lock", false, false, false, true);
    assert!(!root.join("Cargo.lock").exists());
    let report = run_doctor(request(&root, false, false)).expect("doctor report");
    assert!(!root.join("Cargo.lock").exists());
    assert!(report.validation.is_none());
    evolution_test_support::remove_root(&root);
}

#[test]
fn doctor_validate_reports_cargo_lock_side_effect() {
    let root = rust_fixture_with_dependency("doctor-validate-lock");
    let report = run_doctor(request_with_validate(&root, false)).expect("doctor report");
    let validation = report.validation.expect("validation");
    assert!(validation
        .validation_side_effects
        .iter()
        .any(|path| path == "Cargo.lock"));
    assert!(root.join("Cargo.lock").exists());
    evolution_test_support::remove_root(&root);
}

fn request(root: &PathBuf, validate: bool, json: bool) -> DoctorRequest {
    DoctorRequest {
        doctor_id: unique_doctor_id("request"),
        target_path: root.clone(),
        validate,
        json,
        no_llm: true,
        evidence_dir: PathBuf::from(".eva/doctor"),
    }
}

fn request_with_validate(root: &PathBuf, json: bool) -> DoctorRequest {
    DoctorRequest {
        doctor_id: unique_doctor_id("validate"),
        target_path: root.clone(),
        validate: true,
        json,
        no_llm: true,
        evidence_dir: PathBuf::from(".eva/doctor"),
    }
}

fn rust_fixture(
    name: &str,
    ci: bool,
    smoke: bool,
    readme_validation: bool,
    git_clean: bool,
) -> PathBuf {
    let root = evolution_test_support::unique_evolution_root(name);
    fs::create_dir_all(root.join("src")).expect("src");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname=\"doctor_fixture\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
    )
    .expect("cargo");
    fs::write(root.join("src/lib.rs"), "pub fn probe() -> bool { true }\n").expect("lib");
    if ci {
        fs::create_dir_all(root.join(".github/workflows")).expect("workflow dir");
        fs::write(
            root.join(".github/workflows/rust-ci.yml"),
            "name: Rust CI\non: [push]\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - run: cargo fmt --check\n      - run: cargo check --all-targets\n      - run: cargo test\n",
        )
        .expect("ci");
    }
    if smoke {
        fs::create_dir_all(root.join("tests")).expect("tests");
        fs::write(
            root.join("tests/eve_smoke.rs"),
            "#[test]\nfn smoke() { assert!(true); }\n",
        )
        .expect("smoke");
    }
    if readme_validation {
        fs::write(
            root.join("README.md"),
            "# Fixture\n\n## Validation\n\n- cargo fmt --check\n- cargo check\n- cargo test\n",
        )
        .expect("readme");
    } else {
        fs::write(root.join("README.md"), "# Fixture\n").expect("readme");
    }
    if git_clean {
        let _ = std::process::Command::new("git")
            .arg("init")
            .current_dir(&root)
            .output();
    }
    root
}

fn rust_fixture_with_dependency(name: &str) -> PathBuf {
    let root = evolution_test_support::unique_evolution_root(name);
    fs::create_dir_all(root.join("dep/src")).expect("dep src");
    fs::create_dir_all(root.join("src")).expect("src");
    fs::create_dir_all(root.join(".github/workflows")).expect("workflow dir");
    fs::create_dir_all(root.join("tests")).expect("tests dir");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname=\"doctor_validate_fixture\"\nversion=\"0.1.0\"\nedition=\"2021\"\n\n[dependencies]\ndoctor_dep = { path = \"dep\" }\n",
    )
    .expect("cargo");
    fs::write(
        root.join("src/lib.rs"),
        "pub fn probe() -> bool { doctor_dep::probe() }\n",
    )
    .expect("lib");
    fs::write(
        root.join("dep/Cargo.toml"),
        "[package]\nname=\"doctor_dep\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
    )
    .expect("dep cargo");
    fs::write(
        root.join("dep/src/lib.rs"),
        "pub fn probe() -> bool { true }\n",
    )
    .expect("dep lib");
    fs::write(
        root.join(".github/workflows/rust-ci.yml"),
        "name: Rust CI\non: [push]\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - run: cargo fmt --check\n      - run: cargo check --all-targets\n      - run: cargo test\n",
    )
    .expect("ci");
    fs::write(
        root.join("tests/eve_smoke.rs"),
        "#[test]\nfn smoke() { assert!(true); }\n",
    )
    .expect("smoke");
    fs::write(
        root.join("README.md"),
        "# Fixture\n\n## Validation\n\n- cargo fmt --check\n- cargo check\n- cargo test\n",
    )
    .expect("readme");
    root
}

fn unique_doctor_id(name: &str) -> String {
    format!(
        "doctor-test-{}-{}",
        std::process::id(),
        name.replace(|ch: char| !ch.is_ascii_alphanumeric(), "-")
    )
}
