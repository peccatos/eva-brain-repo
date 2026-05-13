use crate::agent::inspect::inspect_workspace;
use crate::agent::storage::{id, memory_path, now_unix, save_json_pretty};
use crate::agent::task::{load_task, update_task};
use crate::contracts::{AgentPlan, AgentTaskStatus, PlanStep};

pub fn plan_task(
    project_root: &str,
    memory_root: &str,
    task_id: &str,
) -> Result<AgentPlan, String> {
    let mut task = load_task(memory_root, task_id)?;
    let inspection = inspect_workspace(project_root, memory_root)?;
    let plan = AgentPlan {
        plan_id: id("plan"),
        task_id: task_id.to_string(),
        generated_at: now_unix(),
        goal: task.goal.clone(),
        planner: "rule_based".into(),
        llm_used: false,
        steps: default_steps(),
        likely_files: likely_files(&task.goal),
        forbidden_paths: task.forbidden_paths.clone(),
        risk_level: task.risk_level.clone(),
        approval_required: true,
        warnings: Vec::new(),
        blockers: Vec::new(),
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
        "plan created\nplan_id={}\ntask_id={}\nplanner={}\nllm_used={}\nsteps={}\nrisk={}\napproval_required={}",
        plan.plan_id,
        plan.task_id,
        plan.planner,
        plan.llm_used,
        plan.steps.len(),
        plan.risk_level,
        plan.approval_required
    ))
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
    let lower = goal.to_ascii_lowercase();
    if lower.contains("test") || lower.contains("провер") {
        vec!["tests/".into()]
    } else if lower.contains("doc")
        || lower.contains("readme")
        || lower.contains("опис")
        || lower.contains("док")
    {
        vec!["docs/".into(), "README.md".into()]
    } else {
        vec!["docs/".into()]
    }
}
