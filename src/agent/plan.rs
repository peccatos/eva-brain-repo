use crate::agent::inspect::inspect_workspace;
use crate::agent::repo_map::build_repo_map;
use crate::agent::storage::{id, memory_path, now_unix, save_json_pretty};
use crate::agent::task::{load_task, update_task};
use crate::contracts::{
    AgentPlan, AgentTask, AgentTaskStatus, LlmPurpose, LlmRequest, LlmResponse, LlmStatus,
    PlanStep, RepoMap,
};
use crate::llm::prompts::AGENT_SYSTEM_PROMPT;
use crate::llm::schemas::AGENT_PLAN_SCHEMA;
use crate::llm::{select_llm_provider_from_env, selected_llm_provider_name_from_env, LlmProvider};

pub fn plan_task(
    project_root: &str,
    memory_root: &str,
    task_id: &str,
) -> Result<AgentPlan, String> {
    let provider_name = selected_llm_provider_name_from_env();
    let provider = select_llm_provider_from_env();
    plan_task_with_provider(
        project_root,
        memory_root,
        task_id,
        provider_name,
        provider.as_ref(),
    )
}

pub fn plan_task_with_provider(
    project_root: &str,
    memory_root: &str,
    task_id: &str,
    provider_name: &str,
    provider: &dyn LlmProvider,
) -> Result<AgentPlan, String> {
    let mut task = load_task(memory_root, task_id)?;
    let inspection = inspect_workspace(project_root, memory_root)?;
    let repo_map = build_repo_map(project_root, memory_root)?;
    let plan = if provider_name == "openai" {
        match build_openai_plan(task_id, &task, &repo_map, provider) {
            Ok(plan) => plan,
            Err(reason) => {
                let mut fallback = build_rule_based_plan(task_id, &task, &repo_map);
                fallback.warnings.push(format!("openai_fallback:{reason}"));
                fallback
            }
        }
    } else {
        build_rule_based_plan(task_id, &task, &repo_map)
    };
    save_json_pretty(
        &memory_path(memory_root, &["plans", &format!("{}.json", plan.plan_id)]),
        &plan,
    )?;
    save_json_pretty(
        &memory_path(memory_root, &["plans", "latest_plan.json"]),
        &plan,
    )?;
    task.status = AgentTaskStatus::Planned;
    task.inspection_id = Some(inspection.inspection_id);
    task.plan_id = Some(plan.plan_id.clone());
    update_task(memory_root, task)?;
    Ok(plan)
}

pub fn print_plan_task(
    project_root: &str,
    memory_root: &str,
    task_id: &str,
) -> Result<String, String> {
    let plan = plan_task(project_root, memory_root, task_id)?;
    Ok(format!(
        "plan created\nplan_id={}\ntask_id={}\nplanner={}\nllm_used={}\nsteps={}\nrisk={}\napproval_required={}\nwarnings={}",
        plan.plan_id,
        plan.task_id,
        plan.planner,
        plan.llm_used,
        plan.steps.len(),
        plan.risk_level,
        plan.approval_required,
        render_warnings(&plan.warnings)
    ))
}

fn build_rule_based_plan(task_id: &str, task: &AgentTask, repo_map: &RepoMap) -> AgentPlan {
    AgentPlan {
        plan_id: id("plan"),
        task_id: task_id.to_string(),
        generated_at: now_unix(),
        goal: task.goal.clone(),
        planner: "rule_based".into(),
        llm_used: false,
        steps: default_steps(),
        likely_files: likely_files_with_repo_map(&task.goal, repo_map),
        forbidden_paths: task.forbidden_paths.clone(),
        risk_level: task.risk_level.clone(),
        approval_required: true,
        warnings: Vec::new(),
        blockers: Vec::new(),
    }
}

fn build_openai_plan(
    task_id: &str,
    task: &AgentTask,
    repo_map: &RepoMap,
    provider: &dyn LlmProvider,
) -> Result<AgentPlan, String> {
    let request = LlmRequest {
        request_id: id("llm-plan"),
        purpose: LlmPurpose::Plan,
        system_prompt: AGENT_SYSTEM_PROMPT.to_string(),
        input: format!(
            "Return JSON for an agent plan.\n\
goal={}\n\
likely_files={}\n\
repo_entrypoints={}\n\
tests={}\n\
docs={}\n\
forbidden_scope_labels={}\n\
Required JSON keys: steps, likely_files, risk_level, warnings, blockers.\n\
Each step must include: title, detail, expected_files, risk.",
            task.goal,
            likely_files_with_repo_map(&task.goal, repo_map).join(","),
            repo_map.entrypoints.join(","),
            repo_map
                .tests
                .iter()
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .join(","),
            repo_map
                .docs
                .iter()
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .join(","),
            summarize_for_llm(&task.forbidden_paths).join(",")
        ),
        expected_schema: AGENT_PLAN_SCHEMA.to_string(),
        max_output_tokens: 1200,
        temperature: 0.0,
    };
    let response = provider.complete(&request)?;
    plan_from_llm_response(task_id, task, repo_map, &response)
}

fn plan_from_llm_response(
    task_id: &str,
    task: &AgentTask,
    repo_map: &RepoMap,
    response: &LlmResponse,
) -> Result<AgentPlan, String> {
    if response.status != LlmStatus::Completed {
        return Err(format_response_reason(response));
    }
    let value = response
        .parsed_json
        .clone()
        .or_else(|| serde_json::from_str(&response.output_text).ok())
        .ok_or_else(|| "malformed_llm_output".to_string())?;
    let steps_value = value
        .get("steps")
        .and_then(|steps| steps.as_array())
        .ok_or_else(|| "malformed_llm_output:steps".to_string())?;
    if steps_value.is_empty() {
        return Err("malformed_llm_output:steps_empty".to_string());
    }
    let steps = steps_value
        .iter()
        .enumerate()
        .map(|(index, step)| PlanStep {
            index: index + 1,
            title: step
                .get("title")
                .and_then(|value| value.as_str())
                .unwrap_or("untitled")
                .to_string(),
            detail: step
                .get("detail")
                .and_then(|value| value.as_str())
                .unwrap_or("")
                .to_string(),
            expected_files: string_array(step.get("expected_files")),
            risk: step
                .get("risk")
                .and_then(|value| value.as_str())
                .unwrap_or("low")
                .to_string(),
        })
        .collect::<Vec<_>>();
    let mut warnings = response.warnings.clone();
    warnings.extend(string_array(value.get("warnings")));
    let blockers = string_array(value.get("blockers"));
    Ok(AgentPlan {
        plan_id: id("plan"),
        task_id: task_id.to_string(),
        generated_at: now_unix(),
        goal: task.goal.clone(),
        planner: response.provider.clone(),
        llm_used: true,
        steps,
        likely_files: {
            let files = string_array(value.get("likely_files"));
            if files.is_empty() {
                likely_files_with_repo_map(&task.goal, repo_map)
            } else {
                files
            }
        },
        forbidden_paths: task.forbidden_paths.clone(),
        risk_level: value
            .get("risk_level")
            .and_then(|value| value.as_str())
            .unwrap_or(&task.risk_level)
            .to_string(),
        approval_required: true,
        warnings,
        blockers,
    })
}

fn string_array(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn render_warnings(warnings: &[String]) -> String {
    if warnings.is_empty() {
        "none".to_string()
    } else {
        warnings.join(",")
    }
}

fn summarize_for_llm(forbidden_paths: &[String]) -> Vec<String> {
    forbidden_paths
        .iter()
        .map(|path| {
            if path.contains(".git") {
                "git_metadata".to_string()
            } else if path.contains("target") {
                "build_artifacts".to_string()
            } else if path.contains("memory") {
                "runtime_memory".to_string()
            } else if path.contains("releases") {
                "release_artifacts".to_string()
            } else if path.contains("sandboxes") {
                "sandbox_artifacts".to_string()
            } else if path.contains(".eva-runtime-tests") || path.contains(".eva-evolution-tests") {
                "isolated_test_roots".to_string()
            } else {
                path.trim_matches('/').replace('/', "_")
            }
        })
        .collect()
}

fn format_response_reason(response: &LlmResponse) -> String {
    let mut parts = vec![format!("llm_status_not_completed:{:?}", response.status)];
    if !response.blockers.is_empty() {
        parts.push(response.blockers.join("|"));
    } else if !response.warnings.is_empty() {
        parts.push(response.warnings.join("|"));
    }
    parts.join(":")
}

pub fn default_steps() -> Vec<PlanStep> {
    [
        (
            "inspect workspace",
            "Read local project metadata and current state.",
        ),
        (
            "identify likely files",
            "Select files that fit task scope and safety policy.",
        ),
        (
            "create patch proposal",
            "Generate structured patch operations without applying them.",
        ),
        (
            "require operator approval",
            "Record approval before file mutation.",
        ),
        (
            "apply safe changes",
            "Use safe path policy and rollback metadata.",
        ),
        (
            "run validation",
            "Run allowlisted cargo validation commands.",
        ),
        (
            "generate evidence report",
            "Persist task evidence and validation status.",
        ),
        (
            "prepare PR summary",
            "Generate local PR summary without pushing.",
        ),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (title, detail))| PlanStep {
        index: index + 1,
        title: title.into(),
        detail: detail.into(),
        expected_files: Vec::new(),
        risk: "low".into(),
    })
    .collect()
}

pub fn likely_files(goal: &str) -> Vec<String> {
    likely_files_with_repo_map(goal, &crate::contracts::RepoMap::default())
}

pub fn likely_files_with_repo_map(goal: &str, repo_map: &crate::contracts::RepoMap) -> Vec<String> {
    let lower = goal.to_ascii_lowercase();
    if lower.contains("test") || lower.contains("провер") {
        if repo_map.tests.is_empty() {
            vec!["tests/".into()]
        } else {
            repo_map.tests.iter().take(3).cloned().collect()
        }
    } else if lower.contains("doc")
        || lower.contains("readme")
        || lower.contains("опис")
        || lower.contains("док")
    {
        let mut files = repo_map.docs.iter().take(3).cloned().collect::<Vec<_>>();
        if files.is_empty() {
            files = vec!["docs/".into(), "README.md".into()];
        }
        files
    } else if lower.contains("cli") || lower.contains("command") || lower.contains("команд") {
        vec![
            "src/main.rs".into(),
            "src/agent/".into(),
            "tests/production_agent_phase16_17_tests.rs".into(),
        ]
    } else {
        vec!["docs/".into()]
    }
}
