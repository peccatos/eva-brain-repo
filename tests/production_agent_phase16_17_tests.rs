use std::fs;
use std::path::PathBuf;

#[path = "evolution_test_support.rs"]
mod evolution_test_support;

use eva_runtime_with_task_validator::{
    add_specimen, approve_proposal, build_agent_report, build_pr_summary_for_task,
    build_production_agent_readiness, create_task, inspect_workspace, list_specimens, list_tasks,
    load_tui_state_from_project_root, plan_task, propose_task, run_validation, show_task,
    AgentTaskStatus, ProposalStatus,
};

#[test]
fn task_creation_writes_agent_task_with_safe_defaults() {
    let root = temp_root("agent-task-create");
    let task = create_task(
        root.join("memory").to_str().unwrap(),
        "document production agent v1",
    )
    .expect("task");
    assert!(root
        .join("memory/tasks")
        .join(format!("{}.json", task.task_id))
        .exists());
    assert_eq!(task.status, AgentTaskStatus::Created);
    assert!(task.approval_required);
    assert!(task.forbidden_paths.iter().any(|path| path == "memory/"));
    assert_eq!(
        list_tasks(root.join("memory").to_str().unwrap())
            .unwrap()
            .len(),
        1
    );
    assert!(show_task(root.join("memory").to_str().unwrap(), "missing")
        .unwrap()
        .is_none());
    evolution_test_support::remove_root(&root);
}

#[test]
fn workspace_inspector_detects_cargo_project_and_handles_git_status() {
    let root = temp_root("agent-inspect");
    let inspection = inspect_workspace(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("inspection");
    assert!(inspection.cargo_project);
    assert!(inspection.cargo_toml_exists);
    assert_eq!(inspection.language, "rust");
    assert!(!inspection.git_status.is_empty());
    evolution_test_support::remove_root(&root);
}

#[test]
fn planner_and_rule_based_proposal_create_deterministic_docs_proposal() {
    let root = temp_root("agent-plan-propose");
    let task = create_task(
        root.join("memory").to_str().unwrap(),
        "document production agent v1",
    )
    .expect("task");
    let plan = plan_task(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        &task.task_id,
    )
    .expect("plan");
    assert_eq!(plan.steps.len(), 8);
    assert!(!plan.llm_used);
    let proposal = propose_task(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        &task.task_id,
    )
    .expect("proposal");
    assert_eq!(proposal.status, ProposalStatus::AwaitingApproval);
    assert_eq!(
        proposal.files_to_change,
        vec![format!("docs/agent_task_{}.md", task.task_id)]
    );
    evolution_test_support::remove_root(&root);
}

#[test]
fn broad_task_is_refused_honestly() {
    let root = temp_root("agent-broad-refusal");
    let task = create_task(root.join("memory").to_str().unwrap(), "make it better").expect("task");
    let proposal = propose_task(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        &task.task_id,
    )
    .expect("proposal");
    assert_eq!(proposal.status, ProposalStatus::Refused);
    assert!(proposal
        .blockers
        .contains(&"task_requires_manual_decomposition".to_string()));
    evolution_test_support::remove_root(&root);
}

#[test]
fn local_agent_loop_reaches_readiness_fixture_state() {
    let root = temp_root("agent-full-loop");
    let memory = root.join("memory");
    let task = create_task(memory.to_str().unwrap(), "document production agent v1").expect("task");
    inspect_workspace(root.to_str().unwrap(), memory.to_str().unwrap()).expect("inspect");
    plan_task(
        root.to_str().unwrap(),
        memory.to_str().unwrap(),
        &task.task_id,
    )
    .expect("plan");
    let proposal = propose_task(
        root.to_str().unwrap(),
        memory.to_str().unwrap(),
        &task.task_id,
    )
    .expect("proposal");
    approve_proposal(memory.to_str().unwrap(), &proposal.proposal_id).expect("approval");
    eva_runtime_with_task_validator::apply_proposal(
        root.to_str().unwrap(),
        memory.to_str().unwrap(),
        &proposal.proposal_id,
    )
    .expect("apply");
    run_validation(root.to_str().unwrap(), memory.to_str().unwrap()).expect("validation");
    build_agent_report(memory.to_str().unwrap(), &task.task_id).expect("report");
    build_pr_summary_for_task(memory.to_str().unwrap(), &task.task_id).expect("pr summary");
    let readiness = build_production_agent_readiness(memory.to_str().unwrap()).expect("readiness");
    assert!(
        readiness.production_agent_v1_ready,
        "{:?}",
        readiness.blockers
    );
    let tui = load_tui_state_from_project_root(&root);
    assert_eq!(tui.agent.task_count, 1);
    assert!(tui.agent.production_agent_v1_ready);
    evolution_test_support::remove_root(&root);
}

#[test]
fn specimen_metadata_does_not_copy_external_source() {
    let root = temp_root("agent-specimen");
    let external = root.join("codex-main");
    fs::create_dir_all(&external).expect("external");
    fs::write(external.join("SOURCE.rs"), "pub fn external() {}\n").expect("external file");
    let metadata = add_specimen(
        root.join("memory").to_str().unwrap(),
        "codex-main",
        external.to_str().unwrap(),
    )
    .expect("specimen");
    assert!(!metadata.source_copy_allowed);
    let specimens = list_specimens(root.join("memory").to_str().unwrap()).expect("list");
    assert_eq!(specimens.len(), 1);
    assert!(!root.join("memory/specimens/SOURCE.rs").exists());
    evolution_test_support::remove_root(&root);
}

fn temp_root(name: &str) -> PathBuf {
    let root = evolution_test_support::unique_evolution_root(name);
    fs::create_dir_all(root.join("src")).expect("src");
    fs::create_dir_all(root.join("tests")).expect("tests");
    fs::create_dir_all(root.join("docs")).expect("docs");
    fs::create_dir_all(root.join("memory")).expect("memory");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname=\"production_agent_temp\"\nversion=\"0.1.0\"\nedition=\"2021\"\n\n[lib]\ndoctest=false\n",
    )
    .expect("cargo");
    fs::write(root.join("src/lib.rs"), "pub fn probe() -> bool { true }\n").expect("lib");
    root
}
