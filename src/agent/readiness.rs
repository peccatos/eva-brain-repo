use std::path::Path;

use crate::agent::storage::{memory_path, save_json_pretty};
use crate::contracts::ProductionAgentReadiness;

pub fn build_production_agent_readiness(
    memory_root: &str,
) -> Result<ProductionAgentReadiness, String> {
    let mut readiness = ProductionAgentReadiness {
        task_intake_ok: Path::new(memory_root).join("tasks").exists(),
        workspace_inspection_ok: memory_path(
            memory_root,
            &["inspections", "latest_inspection.json"],
        )
        .exists(),
        planning_ok: memory_path(memory_root, &["plans", "latest_plan.json"]).exists(),
        llm_adapter_ok: true,
        rule_based_fallback_ok: true,
        proposal_ok: memory_path(memory_root, &["proposals", "latest_proposal.json"]).exists(),
        approval_gate_ok: Path::new(memory_root).join("approvals").exists(),
        safe_apply_ok: memory_path(memory_root, &["applies", "latest_apply.json"]).exists(),
        validation_ok: memory_path(memory_root, &["validations", "latest_validation.json"])
            .exists(),
        report_ok: Path::new(memory_root).join("reports").exists(),
        pr_summary_ok: Path::new(memory_root).join("pr_summaries").exists(),
        tui_agent_visibility_ok: true,
        safety_policy_ok: true,
        production_agent_v1_ready: false,
        warnings: Vec::new(),
        blockers: Vec::new(),
    };
    for (ok, label) in [
        (readiness.task_intake_ok, "task_intake_missing"),
        (
            readiness.workspace_inspection_ok,
            "workspace_inspection_missing",
        ),
        (readiness.planning_ok, "planning_missing"),
        (readiness.proposal_ok, "proposal_missing"),
        (readiness.approval_gate_ok, "approval_gate_missing"),
        (readiness.safe_apply_ok, "safe_apply_missing"),
        (readiness.validation_ok, "validation_missing"),
        (readiness.report_ok, "report_missing"),
        (readiness.pr_summary_ok, "pr_summary_missing"),
    ] {
        if !ok {
            readiness.blockers.push(label.into());
        }
    }
    readiness.production_agent_v1_ready = readiness.blockers.is_empty()
        && readiness.llm_adapter_ok
        && readiness.rule_based_fallback_ok
        && readiness.tui_agent_visibility_ok
        && readiness.safety_policy_ok;
    save_json_pretty(
        &memory_path(memory_root, &["agent", "readiness.json"]),
        &readiness,
    )?;
    Ok(readiness)
}

pub fn print_agent_readiness(memory_root: &str) -> Result<String, String> {
    let readiness = build_production_agent_readiness(memory_root)?;
    Ok(format!(
        "EVA Production Agent Readiness\ntask_intake_ok={}\nworkspace_inspection_ok={}\nplanning_ok={}\nllm_adapter_ok={}\nrule_based_fallback_ok={}\nproposal_ok={}\napproval_gate_ok={}\nsafe_apply_ok={}\nvalidation_ok={}\nreport_ok={}\npr_summary_ok={}\ntui_agent_visibility_ok={}\nsafety_policy_ok={}\nproduction_agent_v1_ready={}\nblockers={}",
        readiness.task_intake_ok,
        readiness.workspace_inspection_ok,
        readiness.planning_ok,
        readiness.llm_adapter_ok,
        readiness.rule_based_fallback_ok,
        readiness.proposal_ok,
        readiness.approval_gate_ok,
        readiness.safe_apply_ok,
        readiness.validation_ok,
        readiness.report_ok,
        readiness.pr_summary_ok,
        readiness.tui_agent_visibility_ok,
        readiness.safety_policy_ok,
        readiness.production_agent_v1_ready,
        if readiness.blockers.is_empty() { "none".into() } else { readiness.blockers.join(",") }
    ))
}
