use std::fs;
use std::path::PathBuf;

#[path = "evolution_test_support.rs"]
mod evolution_test_support;

use eva_runtime_with_task_validator::{
    build_evolution_core_readiness, build_runtime_validation, RuntimeValidation,
};

#[test]
fn phase_16_allowed_is_false_while_runtime_status_is_warn() {
    let root = temp_root("phase160p-readiness-warn");

    let validation = build_runtime_validation(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("validation");
    assert_eq!(validation.status, "warn");

    let readiness = build_evolution_core_readiness(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("readiness");

    assert!(!readiness.runtime_green);
    assert!(!readiness.phase_16_allowed);
    assert!(readiness
        .blockers
        .iter()
        .any(|item| item == "approved_release_candidate_missing"));
    evolution_test_support::remove_root(&root);
}

#[test]
fn phase_16_allowed_requires_green_runtime_validation() {
    let root = temp_root("phase160p-readiness-green");
    let dir = root.join("memory/runtime_validation");
    fs::create_dir_all(&dir).expect("validation dir");
    let validation = RuntimeValidation {
        validation_id: "runtime-validation-green".to_string(),
        status: "green".to_string(),
        approved_release_candidate: Some("ready-run".to_string()),
        release_bundle: Some("release-ready".to_string()),
        metrics_summary: "metrics".to_string(),
        candidate_queue_summary: "queue".to_string(),
        ..RuntimeValidation::default()
    };
    fs::write(
        dir.join("runtime-validation-green.json"),
        serde_json::to_string_pretty(&validation).expect("validation json"),
    )
    .expect("write validation");

    let readiness = build_evolution_core_readiness(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("readiness");

    assert!(readiness.runtime_green);
    assert!(readiness.phase_16_allowed);
    evolution_test_support::remove_root(&root);
}

fn temp_root(name: &str) -> PathBuf {
    let root = evolution_test_support::unique_evolution_root(name);
    fs::create_dir_all(root.join("src")).expect("src");
    fs::create_dir_all(root.join("memory")).expect("memory");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname=\"phase160p_temp\"\nversion=\"0.1.0\"\nedition=\"2021\"\n\n[lib]\ndoctest=false\n",
    )
    .expect("cargo");
    fs::write(root.join("src/lib.rs"), "pub fn probe() -> bool { true }\n").expect("lib");
    fs::write(root.join("memory/regressions.json"), "[]").expect("regressions");
    fs::write(root.join("memory/success_patterns.json"), "[]").expect("success");
    root
}
