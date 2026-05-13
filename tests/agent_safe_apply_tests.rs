use std::fs;
use std::path::{Path, PathBuf};

#[path = "evolution_test_support.rs"]
mod evolution_test_support;

use eva_runtime_with_task_validator::{
    apply_proposal, approve_proposal, create_task, plan_task, propose_task, validate_patch_path,
    ApplyStatus,
};

#[test]
fn safe_path_policy_rejects_forbidden_paths() {
    for path in [
        "../src/main.rs",
        ".git/config",
        "target/debug/foo",
        "memory/tasks/x.json",
        "/etc/passwd",
        "/home/user/file",
        "~/file",
        "src/../../etc/passwd",
    ] {
        assert!(validate_patch_path(path).is_err(), "{path}");
    }
    assert!(validate_patch_path("docs/agent_task.md").is_ok());
}

#[test]
fn apply_refuses_unapproved_proposal() {
    let root = temp_root("agent-safe-apply-unapproved");
    let task = create_task(
        root.join("memory").to_str().unwrap(),
        "document production agent v1",
    )
    .expect("task");
    plan_task(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        &task.task_id,
    )
    .expect("plan");
    let proposal = propose_task(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        &task.task_id,
    )
    .expect("proposal");
    let result = apply_proposal(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        &proposal.proposal_id,
    )
    .expect("apply");
    assert_eq!(result.status, ApplyStatus::Refused);
    assert_eq!(result.blockers, vec!["not_approved"]);
    evolution_test_support::remove_root(&root);
}

#[test]
fn approved_safe_proposal_creates_snapshot_before_writing() {
    let root = temp_root("agent-safe-apply-approved");
    let task = create_task(
        root.join("memory").to_str().unwrap(),
        "document production agent v1",
    )
    .expect("task");
    plan_task(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        &task.task_id,
    )
    .expect("plan");
    let proposal = propose_task(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        &task.task_id,
    )
    .expect("proposal");
    approve_proposal(root.join("memory").to_str().unwrap(), &proposal.proposal_id)
        .expect("approval");
    let result = apply_proposal(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        &proposal.proposal_id,
    )
    .expect("apply");
    assert_eq!(result.status, ApplyStatus::Applied);
    assert!(result
        .snapshot_id
        .as_ref()
        .is_some_and(|path| Path::new(path).exists()));
    assert!(root.join(&result.files_changed[0]).exists());
    evolution_test_support::remove_root(&root);
}

fn temp_root(name: &str) -> PathBuf {
    let root = evolution_test_support::unique_evolution_root(name);
    fs::create_dir_all(root.join("src")).expect("src");
    fs::create_dir_all(root.join("docs")).expect("docs");
    fs::create_dir_all(root.join("memory")).expect("memory");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname=\"agent_safe_apply_temp\"\nversion=\"0.1.0\"\nedition=\"2021\"\n\n[lib]\ndoctest=false\n",
    )
    .expect("cargo");
    fs::write(root.join("src/lib.rs"), "pub fn probe() -> bool { true }\n").expect("lib");
    root
}
