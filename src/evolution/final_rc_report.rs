use std::fs;
use std::path::Path;

use crate::contracts::FinalRcReport;
use crate::evolution::{
    build_runtime_candidate_manifest, build_runtime_cli_contract, build_runtime_service_metadata,
    build_runtime_validation, memory,
};

pub fn build_final_rc_report(
    project_root: &str,
    memory_root: &str,
) -> Result<FinalRcReport, String> {
    let candidate = build_runtime_candidate_manifest(project_root, memory_root)?;
    let validation = build_runtime_validation(project_root, memory_root)?;
    let service = build_runtime_service_metadata(memory_root)?;
    let cli = build_runtime_cli_contract(memory_root)?;
    let generated_at = memory::now_unix();
    let report_id = format!("final-rc-{generated_at}");
    let report_path = Path::new(memory_root)
        .join("final_rc")
        .join(format!("{report_id}.ru.md"));
    let markdown = render_final_rc_markdown(&candidate, &validation, &service, cli.commands.len());
    memory::write_json(
        Path::new(memory_root)
            .join("final_rc")
            .join(format!("{report_id}.json")),
        &serde_json::json!({
            "report_id": report_id,
            "generated_at": generated_at,
            "candidate_id": candidate.candidate_id,
            "validation_id": validation.validation_id,
            "rc_status": candidate.rc_status,
        }),
    )?;
    fs::write(&report_path, markdown)
        .map_err(|error| format!("failed to write final RC markdown: {error}"))?;
    Ok(FinalRcReport {
        report_id,
        generated_at,
        rc_status: candidate.rc_status,
        runtime_candidate_id: candidate.candidate_id,
        runtime_validation_id: validation.validation_id,
        report_path: report_path.display().to_string(),
    })
}

pub fn print_final_rc_report(project_root: &str, memory_root: &str) -> Result<String, String> {
    let candidate = build_runtime_candidate_manifest(project_root, memory_root)?;
    let validation = build_runtime_validation(project_root, memory_root)?;
    let service = build_runtime_service_metadata(memory_root)?;
    let cli = build_runtime_cli_contract(memory_root)?;
    Ok(render_final_rc_markdown(
        &candidate,
        &validation,
        &service,
        cli.commands.len(),
    ))
}

fn render_final_rc_markdown(
    candidate: &crate::contracts::RuntimeCandidateManifest,
    validation: &crate::contracts::RuntimeValidation,
    service: &crate::contracts::RuntimeServiceMetadata,
    cli_commands: usize,
) -> String {
    format!(
        "# EVA Runtime v1.0 Candidate Report\n\nRC status: {}\nCurrent branch/head: {} / {}\nCompleted phases: {}\nPlanned phases: {}\n\n## Safety state\n- auto_promote=false\n- operator approval required=true\n- no network push\n- no merge\n- no self-apply\n- no external repo mutation\n- sandbox_leak_count={}\n\n## Runtime service metadata\n- service_name={}\n- mode={}\n- daemonized={}\n- attach_supported={}\n- watch_supported={}\n- status_supported={}\n- network_required={}\n- network_push_allowed={}\n- external_side_effects={}\n\n## CLI contract summary\n- commands={}\n- metadata_only=true\n\n## Trust and recovery state\n- trust={}\n- preflight_gate_v3={}\n- latest_evidence_bundle_id={}\n- latest_workspace_snapshot_id={}\n- latest_recovery_manifest_id={}\n\n## Release and operations state\n- release={}\n- operations={}\n- ready_candidates={}\n- approved_count={}\n- blocked_candidates={}\n\n## Validation state\n- status={}\n- blockers={}\n- warnings={}\n- green_conditions={}\n- missing_green_conditions={}\n- metrics_summary={}\n- candidate_queue_summary={}\n- sandbox_state={}\n\n## Next operator commands\n{}\n",
        candidate.rc_status,
        candidate.git_branch,
        candidate.git_head,
        if candidate.completed_phases.is_empty() {
            "none".to_string()
        } else {
            candidate.completed_phases.join(", ")
        },
        if candidate.planned_phases.is_empty() {
            "none".to_string()
        } else {
            candidate.planned_phases.join(", ")
        },
        candidate.sandbox_leak_count,
        service.service_name,
        service.mode,
        service.daemonized,
        service.attach_supported,
        service.watch_supported,
        service.status_supported,
        service.network_required,
        service.network_push_allowed,
        service.external_side_effects,
        cli_commands,
        candidate.trust_state,
        candidate.preflight_gate_v3_state,
        candidate.latest_evidence_bundle_id.as_deref().unwrap_or("none"),
        candidate
            .latest_workspace_snapshot_id
            .as_deref()
            .unwrap_or("none"),
        candidate
            .latest_recovery_manifest_id
            .as_deref()
            .unwrap_or("none"),
        candidate.release_state,
        candidate.operations_state,
        candidate.ready_candidates,
        candidate.approved_count,
        candidate.blocked_candidates_count,
        validation.status,
        if validation.blockers.is_empty() {
            "none".to_string()
        } else {
            validation.blockers.join(", ")
        },
        if validation.warnings.is_empty() {
            "none".to_string()
        } else {
            validation.warnings.join(", ")
        },
        if validation.green_conditions.is_empty() {
            "none".to_string()
        } else {
            validation.green_conditions.join(", ")
        },
        if validation.missing_green_conditions.is_empty() {
            "none".to_string()
        } else {
            validation.missing_green_conditions.join(", ")
        },
        validation.metrics_summary.as_str(),
        validation.candidate_queue_summary.as_str(),
        validation.sandbox_state.as_str(),
        validation
            .next_actions
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}
