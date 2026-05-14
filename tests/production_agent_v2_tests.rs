mod agent_v2_support;

use std::fs;
use std::sync::{Mutex, OnceLock};

use agent_v2_support::temp_agent_root;
use eva_runtime_with_task_validator::llm::MockLlmProvider;
use eva_runtime_with_task_validator::{
    build_production_agent_v2_readiness, build_repo_map, create_task, inspect_workspace,
    load_tui_state_from_project_root, plan_task_with_provider, print_agent_v2_readiness,
    propose_task_with_provider, LlmResponse, LlmStatus, ProposalStatus,
};
use serde_json::json;

#[test]
fn git_status_is_never_empty_and_repo_map_exists() {
    let root = temp_agent_root("v2-git-status");
    let memory = root.join("memory");
    let inspection =
        inspect_workspace(root.to_str().unwrap(), memory.to_str().unwrap()).expect("inspection");
    assert!(matches!(
        inspection.git_status.as_str(),
        "clean" | "dirty" | "unknown"
    ));
    let map = build_repo_map(root.to_str().unwrap(), memory.to_str().unwrap()).expect("repo map");
    assert!(map.cargo_project);
    assert!(map.entrypoints.contains(&"src/main.rs".to_string()));
    assert!(map.entrypoints.contains(&"src/lib.rs".to_string()));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn agent_v2_readiness_reports_missing_then_ready_components() {
    let root = temp_agent_root("v2-readiness");
    let memory = root.join("memory");
    let initial = build_production_agent_v2_readiness(memory.to_str().unwrap()).expect("readiness");
    assert!(!initial.production_agent_v2_ready);
    assert!(initial.blockers.contains(&"repo_map_missing".to_string()));
    build_repo_map(root.to_str().unwrap(), memory.to_str().unwrap()).expect("repo map");
    fs::create_dir_all(memory.join("proposals")).expect("proposals");
    fs::write(memory.join("proposals/latest_proposal.json"), "{}").expect("proposal marker");
    fs::create_dir_all(memory.join("plans")).expect("plans");
    fs::write(memory.join("plans/latest_plan.json"), "{}").expect("plan marker");
    fs::create_dir_all(memory.join("task_outcomes")).expect("outcomes");
    fs::create_dir_all(memory.join("validations")).expect("validations");
    fs::write(memory.join("validations/latest_validation.json"), "{}").expect("validation marker");
    let output = print_agent_v2_readiness(memory.to_str().unwrap()).expect("print");
    assert!(output.contains("production_agent_v2_ready=true"));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn tui_loads_agent_v2_state_without_openai_or_writes() {
    let _guard = env_lock().lock().expect("env lock");
    std::env::remove_var("OPENAI_API_KEY");
    std::env::remove_var("EVE_LLM_MODE");
    std::env::remove_var("EVE_LLM_PROVIDER");
    let root = temp_agent_root("v2-tui");
    let memory = root.join("memory");
    build_repo_map(root.to_str().unwrap(), memory.to_str().unwrap()).expect("repo map");
    fs::create_dir_all(memory.join("task_outcomes")).expect("outcomes");
    fs::write(memory.join("task_outcomes/task-a.json"), "{}").expect("outcome marker");
    build_production_agent_v2_readiness(memory.to_str().unwrap()).expect("readiness");
    let state = load_tui_state_from_project_root(&root);
    assert_eq!(state.agent.repo_map_modules, 2);
    assert_eq!(state.agent.task_outcome_count, 1);
    assert_eq!(state.agent.llm_provider, "rule_based");
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn planner_uses_mock_openai_provider_and_falls_back_explicitly() {
    let root = temp_agent_root("v2-plan-provider");
    let memory = root.join("memory");
    let task = create_task(memory.to_str().unwrap(), "document production agent v2").expect("task");

    let openai_plan = plan_task_with_provider(
        root.to_str().unwrap(),
        memory.to_str().unwrap(),
        &task.task_id,
        "openai",
        &MockLlmProvider {
            response: LlmResponse {
                request_id: String::new(),
                provider: "openai".to_string(),
                model: "gpt-5.5".to_string(),
                status: LlmStatus::Completed,
                output_text: String::new(),
                parsed_json: Some(json!({
                    "steps": [{
                        "title": "inspect workspace",
                        "detail": "Review local files.",
                        "expected_files": ["docs/"],
                        "risk": "low"
                    }],
                    "likely_files": ["docs/example.md"],
                    "risk_level": "low"
                })),
                warnings: Vec::new(),
                blockers: Vec::new(),
            },
        },
    )
    .expect("openai plan");
    assert_eq!(openai_plan.planner, "openai");
    assert!(openai_plan.llm_used);

    let fallback_plan = plan_task_with_provider(
        root.to_str().unwrap(),
        memory.to_str().unwrap(),
        &task.task_id,
        "openai",
        &MockLlmProvider {
            response: LlmResponse {
                request_id: String::new(),
                provider: "openai".to_string(),
                model: "gpt-5.5".to_string(),
                status: LlmStatus::Failed,
                output_text: String::new(),
                parsed_json: None,
                warnings: Vec::new(),
                blockers: vec!["openai_http_status:500".to_string()],
            },
        },
    )
    .expect("fallback plan");
    assert_eq!(fallback_plan.planner, "rule_based");
    assert!(!fallback_plan.llm_used);
    assert!(fallback_plan
        .warnings
        .iter()
        .any(|item| item.starts_with("openai_fallback:")));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn proposer_uses_mock_openai_provider_and_falls_back_explicitly() {
    let root = temp_agent_root("v2-propose-provider");
    let memory = root.join("memory");
    let task = create_task(memory.to_str().unwrap(), "document production agent v2").expect("task");

    let openai_proposal = propose_task_with_provider(
        root.to_str().unwrap(),
        memory.to_str().unwrap(),
        &task.task_id,
        "openai",
        &MockLlmProvider {
            response: LlmResponse {
                request_id: String::new(),
                provider: "openai".to_string(),
                model: "gpt-5.5".to_string(),
                status: LlmStatus::Completed,
                output_text: String::new(),
                parsed_json: Some(json!({
                    "summary": "docs proposal",
                    "files_to_change": ["docs/example.md"],
                    "risk_level": "low",
                    "patch_ops": [{
                        "path": "docs/example.md",
                        "op": "CreateFile",
                        "description": "doc",
                        "content": "# Example\n"
                    }]
                })),
                warnings: Vec::new(),
                blockers: Vec::new(),
            },
        },
    )
    .expect("openai proposal");
    assert_eq!(openai_proposal.proposer, "openai");
    assert!(openai_proposal.llm_used);
    assert_eq!(openai_proposal.status, ProposalStatus::AwaitingApproval);

    let fallback_proposal = propose_task_with_provider(
        root.to_str().unwrap(),
        memory.to_str().unwrap(),
        &task.task_id,
        "openai",
        &MockLlmProvider {
            response: LlmResponse {
                request_id: String::new(),
                provider: "openai".to_string(),
                model: "gpt-5.5".to_string(),
                status: LlmStatus::Completed,
                output_text: "{\"summary\":\"broken\"}".to_string(),
                parsed_json: Some(json!({
                    "summary": "broken"
                })),
                warnings: Vec::new(),
                blockers: Vec::new(),
            },
        },
    )
    .expect("fallback proposal");
    assert_eq!(fallback_proposal.proposer, "rule_based");
    assert!(!fallback_proposal.llm_used);
    assert!(fallback_proposal
        .warnings
        .iter()
        .any(|item| item.starts_with("openai_fallback:")));
    fs::remove_dir_all(root).expect("cleanup");
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
