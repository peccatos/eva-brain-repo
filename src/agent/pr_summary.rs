use std::fs;

use crate::agent::storage::{id, memory_path, now_unix, save_json_pretty};
use crate::agent::task::{load_task, update_task};
use crate::contracts::{AgentTaskStatus, PrSummary};

pub fn build_pr_summary_for_task(memory_root: &str, task_id: &str) -> Result<PrSummary, String> {
    let mut task = load_task(memory_root, task_id)?;
    let summary = PrSummary {
        pr_summary_id: id("pr-summary"),
        task_id: task_id.into(),
        generated_at: now_unix(),
        title: format!("EVE agent task: {}", task.goal),
        body: render_body(&task.goal),
        validation_checklist: vec![
            "cargo fmt --check".into(),
            "cargo check".into(),
            "cargo test".into(),
        ],
        risk_notes: vec!["metadata-only PR summary; no push performed".into()],
    };
    save_json_pretty(
        &memory_path(memory_root, &["pr_summaries", &format!("{task_id}.json")]),
        &summary,
    )?;
    fs::write(
        memory_path(memory_root, &["pr_summaries", &format!("{task_id}.md")]),
        &summary.body,
    )
    .map_err(|error| format!("write pr summary markdown: {error}"))?;
    task.status = AgentTaskStatus::Completed;
    task.pr_summary_id = Some(summary.pr_summary_id.clone());
    update_task(memory_root, task)?;
    Ok(summary)
}

pub fn print_pr_summary_for_task(memory_root: &str, task_id: &str) -> Result<String, String> {
    let summary = build_pr_summary_for_task(memory_root, task_id)?;
    Ok(format!(
        "PR Title:\n{}\n\nPR Body:\n{}",
        summary.title, summary.body
    ))
}

fn render_body(goal: &str) -> String {
    format!(
        "## Summary\n\n{}\n\n## Changes\n\n- Generated through EVE governed production agent flow.\n\n## Validation\n\n- cargo fmt --check\n- cargo check\n- cargo test\n\n## Safety\n\n- auto_promote=false\n- operator approval required\n- no git push\n- no git merge\n\n## Risks\n\n- Operator must review final diff.\n\n## Notes\n\nGenerated locally; no PR was created.\n",
        goal
    )
}
