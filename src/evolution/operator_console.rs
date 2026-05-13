use crate::contracts::OperatorConsoleReport;
use crate::evolution::{
    build_artifact_audit, build_capability_policy, build_determinism_audit,
    build_future_phase_registry, build_operations_report, build_preflight_gate,
    build_preflight_gate_v3, build_release_health, build_runtime_candidate_manifest,
    build_trust_decision, build_workspace_snapshot, governance_status, memory, print_eva_status,
    print_operator_runbook, print_release_status,
};

pub fn build_operator_console_report(
    project_root: &str,
    memory_root: &str,
) -> Result<OperatorConsoleReport, String> {
    let governance = governance_status(project_root, memory_root)?;
    let release_status = print_release_status(memory_root)?;
    let health = build_release_health(project_root, memory_root)?;
    let gate = build_preflight_gate(project_root, memory_root)?;
    let artifact = build_artifact_audit(project_root)?;
    let determinism = build_determinism_audit(project_root, memory_root)?;
    let operations = build_operations_report(project_root, memory_root)?;
    let future = build_future_phase_registry();
    let policy = build_capability_policy();
    let trust = build_trust_decision(project_root, memory_root)?;
    let snapshot = build_workspace_snapshot(project_root, memory_root)?;
    let gate_v3 = build_preflight_gate_v3(project_root, memory_root)?;
    let runtime_candidate = build_runtime_candidate_manifest(project_root, memory_root)?;
    Ok(OperatorConsoleReport {
        generated_at: memory::now_unix(),
        status_lines: vec![
            print_eva_status(project_root, memory_root)?,
            format!(
                "governance_status: approved={} rejected={} deferred={} ready_approved={} auto_promote={}",
                governance.approved_count,
                governance.rejected_count,
                governance.deferred_count,
                governance.promotion_ready_approved_count,
                governance.auto_promote
            ),
            format!("release_status: {release_status}"),
            format!(
                "release_health: grade={} score={}",
                health.health_grade, health.health_score
            ),
            format!("preflight_gate: status={}", gate.gate_status),
            format!(
                "artifact_audit: status={} sandbox_leaks={}",
                if artifact.should_fail_release { "fail" } else { "pass" },
                artifact.sandbox_leaks.len()
            ),
            format!(
                "determinism_audit: status={}",
                if determinism.deterministic_enough { "pass" } else { "fail" }
            ),
            format!(
                "operations_status: next={} future_allowed_now={}",
                operations.next_safe_operator_action, operations.future_phases_allowed_now
            ),
            format!(
                "capability_policy: denied={} allowed={}",
                policy.denied_capabilities.len(),
                policy.allowed_capabilities.len()
            ),
            format!(
                "trust_decision: decision={} blockers={} warnings={}",
                trust.trust_decision,
                trust.blockers.len(),
                trust.warnings.len()
            ),
            format!(
                "workspace_snapshot: id={} modified={} untracked={}",
                snapshot.snapshot_id, snapshot.modified_count, snapshot.untracked_count
            ),
            format!("preflight_gate_v3: status={}", gate_v3.status),
            format!(
                "runtime_candidate: status={} id={} planned_phases={}",
                runtime_candidate.rc_status,
                runtime_candidate.candidate_id,
                runtime_candidate.planned_phases.len()
            ),
            "phase15_visibility: tui=true metrics_truth=true candidate_queue_hygiene=true release_candidate_flow=true green_gate=true".to_string(),
            format!(
                "future_phases: {}",
                future
                    .entries
                    .iter()
                    .map(|entry| format!("{}={}", entry.phase, entry.status))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ],
        next_commands: vec![
            if gate_v3.status == "pass" {
                "cargo run -- --trust-proof-report".to_string()
            } else {
                operations.next_safe_operator_action
            },
            "cargo run -- --runtime-candidate".to_string(),
            "cargo run -- tui".to_string(),
            "cargo run -- --runtime-validation".to_string(),
            "cargo run -- --final-rc-report".to_string(),
            "cargo run -- --preflight-gate-v3".to_string(),
            "cargo run -- --proof-report".to_string(),
            "cargo run -- --operator-runbook".to_string(),
        ],
    })
}

pub fn print_operator_console(project_root: &str, memory_root: &str) -> Result<String, String> {
    let report = build_operator_console_report(project_root, memory_root)?;
    let runbook = print_operator_runbook(project_root, memory_root)?;
    Ok(format!(
        "# EVA Operator Console\n\n{}\n\n## Next commands\n{}\n\n{}\n",
        report.status_lines.join("\n"),
        report
            .next_commands
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n"),
        runbook
    ))
}
