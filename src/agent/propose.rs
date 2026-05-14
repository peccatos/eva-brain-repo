use crate::agent::outcome::build_task_outcome;
use crate::agent::plan::plan_task;
use crate::agent::safe_paths::validate_patch_path;
use crate::agent::storage::{id, load_json, memory_path, now_unix, save_json_pretty};
use crate::agent::task::{load_task, update_task};
use crate::contracts::{
    AgentPlan, AgentTask, AgentTaskStatus, LlmPurpose, LlmRequest, LlmResponse, LlmStatus, PatchOp,
    PatchOperationKind, PatchProposal, ProposalStatus,
};
use crate::llm::prompts::AGENT_SYSTEM_PROMPT;
use crate::llm::schemas::PATCH_PROPOSAL_SCHEMA;
use crate::llm::{select_llm_provider_from_env, selected_llm_provider_name_from_env, LlmProvider};

pub const MAX_FILES_PER_PROPOSAL: usize = 5;
pub const MAX_PATCH_OPS_PER_PROPOSAL: usize = 10;
pub const MAX_CONTENT_PER_OP: usize = 20 * 1024;
pub const MAX_TOTAL_PROPOSAL_CONTENT: usize = 80 * 1024;

pub fn propose_task(
    project_root: &str,
    memory_root: &str,
    task_id: &str,
) -> Result<PatchProposal, String> {
    let provider_name = selected_llm_provider_name_from_env();
    let provider = select_llm_provider_from_env();
    propose_task_with_provider(
        project_root,
        memory_root,
        task_id,
        provider_name,
        provider.as_ref(),
    )
}

pub fn propose_task_with_provider(
    project_root: &str,
    memory_root: &str,
    task_id: &str,
    provider_name: &str,
    provider: &dyn LlmProvider,
) -> Result<PatchProposal, String> {
    let mut task = load_task(memory_root, task_id)?;
    let plan = match task.plan_id.clone() {
        Some(plan_id) => load_json(&memory_path(
            memory_root,
            &["plans", &format!("{plan_id}.json")],
        ))?,
        None => plan_task(project_root, memory_root, task_id)?,
    };
    let mut proposal = if provider_name == "openai" {
        match build_openai_proposal(memory_root, task_id, &task, &plan, provider) {
            Ok(proposal) => proposal,
            Err(reason) => {
                let mut fallback = build_rule_based_proposal(task_id, &task, &plan);
                fallback.warnings.push(format!("openai_fallback:{reason}"));
                fallback
            }
        }
    } else {
        build_rule_based_proposal(task_id, &task, &plan)
    };
    validate_patch_proposal(&mut proposal);
    save_json_pretty(
        &memory_path(
            memory_root,
            &["proposals", &format!("{}.json", proposal.proposal_id)],
        ),
        &proposal,
    )?;
    save_json_pretty(
        &memory_path(memory_root, &["proposals", "latest_proposal.json"]),
        &proposal,
    )?;
    task.status = if proposal.status == ProposalStatus::Refused {
        AgentTaskStatus::Blocked
    } else {
        AgentTaskStatus::Proposed
    };
    task.proposal_id = Some(proposal.proposal_id.clone());
    update_task(memory_root, task)?;
    if proposal.status == ProposalStatus::Refused {
        let _ = build_task_outcome(memory_root, task_id);
    }
    Ok(proposal)
}

pub fn validate_patch_proposal(proposal: &mut PatchProposal) {
    if proposal.files_to_change.len() > MAX_FILES_PER_PROPOSAL
        || proposal.patch_ops.len() > MAX_PATCH_OPS_PER_PROPOSAL
    {
        proposal.blockers.push("patch_too_large".to_string());
    }
    let mut total_content_size = 0usize;
    for op in &proposal.patch_ops {
        if let Err(error) = validate_patch_path(&op.path) {
            proposal.blockers.push(error.to_string());
        }
        let content_size = op.content.as_ref().map(|value| value.len()).unwrap_or(0)
            + op.find.as_ref().map(|value| value.len()).unwrap_or(0)
            + op.replace.as_ref().map(|value| value.len()).unwrap_or(0);
        total_content_size += content_size;
        if content_size > MAX_CONTENT_PER_OP {
            proposal.blockers.push("patch_too_large".to_string());
        }
        if matches!(op.op, PatchOperationKind::ReplaceExactText)
            && (op.find.as_deref().unwrap_or_default().is_empty()
                || op.replace.as_deref().unwrap_or_default().is_empty())
        {
            proposal
                .blockers
                .push(format!("invalid_replace_exact_text:{}", op.path));
        }
        if matches!(
            op.op,
            PatchOperationKind::CreateFile
                | PatchOperationKind::AppendFile
                | PatchOperationKind::ReplaceFileIfExists
        ) && op.content.is_none()
        {
            proposal
                .blockers
                .push(format!("missing_content:{}", op.path));
        }
    }
    if total_content_size > MAX_TOTAL_PROPOSAL_CONTENT {
        proposal.blockers.push("patch_too_large".to_string());
    }
    if proposal.approved && proposal.approved_at.is_none() {
        proposal.blockers.push("proposal_self_approved".into());
    }
    proposal.blockers.sort();
    proposal.blockers.dedup();
    if !proposal.blockers.is_empty() {
        proposal.status = ProposalStatus::Refused;
    }
}

pub fn load_proposal(memory_root: &str, proposal_id: &str) -> Result<PatchProposal, String> {
    load_json(&memory_path(
        memory_root,
        &["proposals", &format!("{proposal_id}.json")],
    ))
}

pub fn save_proposal(memory_root: &str, proposal: &PatchProposal) -> Result<(), String> {
    save_json_pretty(
        &memory_path(
            memory_root,
            &["proposals", &format!("{}.json", proposal.proposal_id)],
        ),
        proposal,
    )?;
    save_json_pretty(
        &memory_path(memory_root, &["proposals", "latest_proposal.json"]),
        proposal,
    )
}

pub fn proposal_from_llm_response(
    memory_root: &str,
    task_id: &str,
    plan_id: &str,
    goal: &str,
    response: &LlmResponse,
) -> Result<PatchProposal, String> {
    if response.status != LlmStatus::Completed {
        return Err(format_response_reason(response));
    }
    let value = response
        .parsed_json
        .clone()
        .or_else(|| serde_json::from_str(&response.output_text).ok())
        .ok_or_else(|| "malformed_llm_output".to_string())?;
    let mut blockers = Vec::new();
    if value.get("approved").and_then(|value| value.as_bool()) == Some(true)
        || value.get("apply").is_some()
        || value.get("shell_commands").is_some()
    {
        blockers.push("llm_attempted_gate_bypass".to_string());
    }
    let summary = value
        .get("summary")
        .and_then(|value| value.as_str())
        .unwrap_or("Structured LLM patch proposal.")
        .to_string();
    let risk_level = value
        .get("risk_level")
        .and_then(|value| value.as_str())
        .unwrap_or("low")
        .to_string();
    let files_to_change = value
        .get("files_to_change")
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let patch_ops = value
        .get("patch_ops")
        .and_then(|value| value.as_array())
        .ok_or_else(|| "malformed_llm_output".to_string())?
        .iter()
        .map(parse_patch_op)
        .collect::<Result<Vec<_>, _>>()?;
    let mut proposal = PatchProposal {
        proposal_id: id("proposal"),
        task_id: task_id.to_string(),
        plan_id: plan_id.to_string(),
        status: ProposalStatus::AwaitingApproval,
        created_at: now_unix(),
        updated_at: now_unix(),
        goal: goal.to_string(),
        summary,
        proposer: response.provider.clone(),
        llm_used: response.provider != "rule_based",
        files_to_change,
        forbidden_paths: vec![
            ".git/".to_string(),
            "target/".to_string(),
            "memory/".to_string(),
            "releases/".to_string(),
            "sandboxes/".to_string(),
        ],
        risk_level,
        approval_required: true,
        approved: false,
        approved_at: None,
        patch_ops,
        warnings: response.warnings.clone(),
        blockers,
    };
    if proposal.files_to_change.is_empty() {
        proposal.files_to_change = proposal
            .patch_ops
            .iter()
            .map(|op| op.path.clone())
            .collect();
    }
    validate_patch_proposal(&mut proposal);
    save_proposal(memory_root, &proposal)?;
    Ok(proposal)
}

fn parse_patch_op(value: &serde_json::Value) -> Result<PatchOp, String> {
    let path = value
        .get("path")
        .and_then(|value| value.as_str())
        .ok_or_else(|| "malformed_llm_output:path".to_string())?
        .to_string();
    let op = match value
        .get("op")
        .and_then(|value| value.as_str())
        .unwrap_or("")
    {
        "CreateFile" => PatchOperationKind::CreateFile,
        "AppendFile" => PatchOperationKind::AppendFile,
        "ReplaceFileIfExists" => PatchOperationKind::ReplaceFileIfExists,
        "ReplaceExactText" => PatchOperationKind::ReplaceExactText,
        _ => return Err("malformed_llm_output:op".to_string()),
    };
    Ok(PatchOp {
        path,
        op,
        description: value
            .get("description")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .to_string(),
        content: value
            .get("content")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        find: value
            .get("find")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        replace: value
            .get("replace")
            .and_then(|value| value.as_str())
            .map(str::to_string),
    })
}

pub fn print_propose_task(
    project_root: &str,
    memory_root: &str,
    task_id: &str,
) -> Result<String, String> {
    let proposal = propose_task(project_root, memory_root, task_id)?;
    if proposal.status == ProposalStatus::Refused {
        return Ok(format!(
            "proposal refused\ntask_id={}\nreason={}\nblockers={}\nwarnings={}",
            proposal.task_id,
            proposal
                .blockers
                .first()
                .map(String::as_str)
                .unwrap_or("unknown"),
            proposal.blockers.join(","),
            render_warnings(&proposal.warnings)
        ));
    }
    Ok(format!(
        "proposal created\nproposal_id={}\ntask_id={}\nproposer={}\nllm_used={}\nstatus=AwaitingApproval\nfiles_to_change={}\napproval_required=true\nwarnings={}",
        proposal.proposal_id,
        proposal.task_id,
        proposal.proposer,
        proposal.llm_used,
        proposal.files_to_change.join(","),
        render_warnings(&proposal.warnings)
    ))
}

pub fn print_proposal_show(memory_root: &str, proposal_id: &str) -> Result<String, String> {
    match load_proposal(memory_root, proposal_id) {
        Ok(mut proposal) => {
            validate_patch_proposal(&mut proposal);
            Ok(format!(
                "EVA Patch Proposal\nproposal_id={}\ntask_id={}\nstatus={:?}\nproposer={}\nllm_used={}\nrisk_level={}\napproval_required={}\napproved={}\nfiles_to_change={}\npatch_ops={}\nwarnings={}\nblockers={}",
                proposal.proposal_id,
                proposal.task_id,
                proposal.status,
                proposal.proposer,
                proposal.llm_used,
                proposal.risk_level,
                proposal.approval_required,
                proposal.approved,
                proposal.files_to_change.join(","),
                proposal
                    .patch_ops
                    .iter()
                    .map(|op| format!("{}:{:?}", op.path, op.op))
                    .collect::<Vec<_>>()
                    .join(","),
                if proposal.warnings.is_empty() { "none".to_string() } else { proposal.warnings.join(",") },
                if proposal.blockers.is_empty() { "none".to_string() } else { proposal.blockers.join(",") }
            ))
        }
        Err(_) => Ok(format!("proposal not found\nproposal_id={proposal_id}")),
    }
}

fn build_rule_based_proposal(task_id: &str, task: &AgentTask, plan: &AgentPlan) -> PatchProposal {
    let lower = task.goal.to_ascii_lowercase();
    let mut blockers = Vec::new();
    let patch_ops = if lower.contains("doc")
        || lower.contains("readme")
        || lower.contains("опис")
        || lower.contains("док")
    {
        let path = format!("docs/agent_task_{task_id}.md");
        vec![PatchOp {
            path,
            op: PatchOperationKind::CreateFile,
            description: "Create deterministic task documentation.".into(),
            content: Some(format!(
                "# Agent Task {task_id}\n\nGoal: {}\n\nGenerated by EVE governed production agent.\n",
                task.goal
            )),
            find: None,
            replace: None,
        }]
    } else if lower.contains("test") || lower.contains("tests") || lower.contains("провер") {
        let safe_task = task_id.replace('-', "_");
        vec![PatchOp {
            path: format!("tests/agent_generated_{safe_task}_tests.rs"),
            op: PatchOperationKind::CreateFile,
            description: "Create deterministic compile-safe agent test fixture.".into(),
            content: Some(format!(
                "#[test]\nfn agent_generated_{safe_task}_compiles() {{\n    assert!(true);\n}}\n"
            )),
            find: None,
            replace: None,
        }]
    } else {
        blockers.push("task_requires_manual_decomposition".into());
        Vec::new()
    };
    PatchProposal {
        proposal_id: id("proposal"),
        task_id: task_id.into(),
        plan_id: plan.plan_id.clone(),
        status: if blockers.is_empty() {
            ProposalStatus::AwaitingApproval
        } else {
            ProposalStatus::Refused
        },
        created_at: now_unix(),
        updated_at: now_unix(),
        goal: task.goal.clone(),
        summary: "Deterministic rule-based patch proposal.".into(),
        proposer: "rule_based".into(),
        llm_used: false,
        files_to_change: patch_ops.iter().map(|op| op.path.clone()).collect(),
        forbidden_paths: task.forbidden_paths.clone(),
        risk_level: task.risk_level.clone(),
        approval_required: true,
        approved: false,
        approved_at: None,
        patch_ops,
        warnings: Vec::new(),
        blockers,
    }
}

fn build_openai_proposal(
    memory_root: &str,
    task_id: &str,
    task: &AgentTask,
    plan: &AgentPlan,
    provider: &dyn LlmProvider,
) -> Result<PatchProposal, String> {
    let request = LlmRequest {
        request_id: id("llm-proposal"),
        purpose: LlmPurpose::ProposePatch,
        system_prompt: AGENT_SYSTEM_PROMPT.to_string(),
        input: format!(
            "Return JSON for a patch proposal.\n\
goal={}\n\
plan_steps={}\n\
likely_files={}\n\
forbidden_scope_labels={}\n\
allowed_ops=CreateFile,AppendFile,ReplaceFileIfExists,ReplaceExactText\n\
approval_required=true\n\
Never approve, never apply, never include shell commands.",
            task.goal,
            plan.steps
                .iter()
                .map(|step| step.title.clone())
                .collect::<Vec<_>>()
                .join(","),
            plan.likely_files.join(","),
            summarize_for_llm(&task.forbidden_paths).join(",")
        ),
        expected_schema: PATCH_PROPOSAL_SCHEMA.to_string(),
        max_output_tokens: 1800,
        temperature: 0.0,
    };
    let response = provider.complete(&request)?;
    proposal_from_llm_response(memory_root, task_id, &plan.plan_id, &task.goal, &response)
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
