use std::fs;

use crate::agent::storage::{id, load_json, memory_path, now_unix, save_json_pretty};
use crate::agent::task::{load_task, update_task};
use crate::contracts::{AgentReport, AgentTaskStatus, ApplyResult, ValidationRun};

pub fn build_agent_report(memory_root: &str, task_id: &str) -> Result<AgentReport, String> {
    let mut task = load_task(memory_root, task_id)?;
    let apply = task.apply_id.as_ref().and_then(|id| {
        load_json::<ApplyResult>(&memory_path(
            memory_root,
            &["applies", &format!("{id}.json")],
        ))
        .ok()
    });
    let validation = load_json::<ValidationRun>(&memory_path(
        memory_root,
        &["validations", "latest_validation.json"],
    ))
    .ok();
    let report = AgentReport {
        report_id: id("agent-report"),
        task_id: task_id.into(),
        generated_at: now_unix(),
        goal: task.goal.clone(),
        task_status: format!("{:?}", task.status),
        inspection_id: task.inspection_id.clone(),
        plan_id: task.plan_id.clone(),
        proposal_id: task.proposal_id.clone(),
        approval_id: task.approval_id.clone(),
        apply_id: task.apply_id.clone(),
        validation_id: validation.as_ref().map(|v| v.validation_id.clone()),
        files_changed: apply.map(|a| a.files_changed).unwrap_or_default(),
        validation_status: validation
            .as_ref()
            .map(|v| format!("{:?}", v.status).to_ascii_lowercase())
            .unwrap_or_else(|| "not_run".into()),
        summary: "Governed local agent task evidence report.".into(),
        risks: vec!["operator approval required".into()],
        next_actions: vec!["review PR summary".into()],
        warnings: Vec::new(),
        blockers: Vec::new(),
    };
    save_json_pretty(
        &memory_path(memory_root, &["reports", &format!("agent-{task_id}.json")]),
        &report,
    )?;
    fs::write(
        memory_path(memory_root, &["reports", &format!("agent-{task_id}.md")]),
        render_report(&report),
    )
    .map_err(|error| format!("write agent report markdown: {error}"))?;
    task.status = AgentTaskStatus::Reported;
    task.report_id = Some(report.report_id.clone());
    update_task(memory_root, task)?;
    Ok(report)
}

pub fn print_agent_report(memory_root: &str, task_id: &str) -> Result<String, String> {
    let report = build_agent_report(memory_root, task_id)?;
    Ok(format!(
        "agent report generated\ntask_id={}\nreport_id={}\nvalidation_status={}",
        report.task_id, report.report_id, report.validation_status
    ))
}

pub fn render_report(report: &AgentReport) -> String {
    format!(
        "# EVE Agent Report\n\n## Task\n{}\n\n## Workspace\ninspection_id={}\n\n## Plan\nplan_id={}\n\n## Proposal\nproposal_id={}\n\n## Approval\napproval_id={}\n\n## Apply Result\napply_id={}\n\n## Validation\n{}\n\n## Files Changed\n{}\n\n## Risks\n{}\n\n## Blockers\n{}\n\n## Next Actions\n{}\n",
        report.goal,
        report.inspection_id.as_deref().unwrap_or("missing"),
        report.plan_id.as_deref().unwrap_or("missing"),
        report.proposal_id.as_deref().unwrap_or("missing"),
        report.approval_id.as_deref().unwrap_or("missing"),
        report.apply_id.as_deref().unwrap_or("missing"),
        report.validation_status,
        report.files_changed.join(","),
        report.risks.join(","),
        report.blockers.join(","),
        report.next_actions.join(",")
    )
}
