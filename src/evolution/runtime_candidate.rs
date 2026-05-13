use std::path::Path;
use std::process::Command;

use crate::contracts::{ProofReport, RuntimeCandidateManifest};
use crate::evolution::{
    build_future_phase_registry, build_operations_report, build_preflight_gate_v3,
    build_proof_report, build_runtime_validation, build_trust_decision, build_workspace_snapshot,
    governance_status, latest_evidence_bundle_id, latest_proof_snapshot_id,
    latest_recovery_manifest_id, latest_workspace_snapshot_id, memory, print_release_status,
};

pub fn build_runtime_candidate_manifest(
    project_root: &str,
    memory_root: &str,
) -> Result<RuntimeCandidateManifest, String> {
    let proof = build_proof_report(project_root, memory_root)?;
    let governance = governance_status(project_root, memory_root)?;
    let release_state = print_release_status(memory_root)?;
    let trust = build_trust_decision(project_root, memory_root)?;
    let operations = build_operations_report(project_root, memory_root)?;
    let gate_v3 = build_preflight_gate_v3(project_root, memory_root)?;
    let validation = build_runtime_validation(project_root, memory_root)?;
    let snapshot = build_workspace_snapshot(project_root, memory_root)?;
    let future = build_future_phase_registry();
    let generated_at = memory::now_unix();

    let manifest = RuntimeCandidateManifest {
        candidate_id: format!("runtime-candidate-{generated_at}"),
        generated_at,
        git_branch: git_stdout(project_root, &["branch", "--show-current"])
            .unwrap_or_else(|| "unknown".to_string()),
        git_head: git_stdout(project_root, &["rev-parse", "HEAD"])
            .unwrap_or_else(|| "unknown".to_string()),
        completed_phases: future
            .entries
            .iter()
            .filter(|entry| entry.status.starts_with("completed_by_"))
            .map(|entry| format!("{} {}", entry.phase, entry.name))
            .collect(),
        planned_phases: future
            .entries
            .iter()
            .filter(|entry| entry.status == "planned")
            .map(|entry| format!("{} {}", entry.phase, entry.name))
            .collect(),
        support_flags: proof_support_flags(&proof),
        governance_state: format!(
            "approved={} rejected={} deferred={} ready_approved={}",
            governance.approved_count,
            governance.rejected_count,
            governance.deferred_count,
            governance.promotion_ready_approved_count
        ),
        release_state,
        trust_state: format!(
            "decision={} blockers={} warnings={}",
            trust.trust_decision,
            trust.blockers.len(),
            trust.warnings.len()
        ),
        operations_state: format!(
            "health={} preflight={} next={}",
            operations.release_health_grade,
            operations.preflight_gate_status,
            operations.next_safe_operator_action
        ),
        preflight_gate_v3_state: format!(
            "status={} blockers={} warnings={}",
            gate_v3.status,
            gate_v3.blockers.len(),
            gate_v3.warnings.len()
        ),
        auto_promote: false,
        operator_approval_required: true,
        sandbox_leak_count: snapshot.sandbox_leak_count,
        release_count: proof.release_count,
        ready_candidates: proof.ready_candidates,
        approved_count: governance.approved_count,
        blocked_candidates_count: proof.blocked_candidates,
        latest_evidence_bundle_id: latest_evidence_bundle_id(memory_root)?,
        latest_workspace_snapshot_id: latest_workspace_snapshot_id(memory_root)?,
        latest_recovery_manifest_id: latest_recovery_manifest_id(memory_root)?,
        latest_proof_snapshot_id: latest_proof_snapshot_id(memory_root)?,
        latest_bounded_run_id: proof.latest_bounded_run_id.clone(),
        latest_supervised_run_id: proof.latest_supervised_run_id.clone(),
        rc_status: if validation.status == "blocked" {
            "blocked".to_string()
        } else if governance.promotion_ready_approved_count == 0
            || validation.status == "warn"
            || gate_v3.status == "warn"
        {
            "warn".to_string()
        } else if validation.status == "green" {
            "pass".to_string()
        } else {
            "warn".to_string()
        },
    };
    memory::write_json(
        Path::new(memory_root)
            .join("runtime_candidates")
            .join(format!("{}.json", manifest.candidate_id)),
        &manifest,
    )?;
    Ok(manifest)
}

pub fn print_runtime_candidate(project_root: &str, memory_root: &str) -> Result<String, String> {
    serde_json::to_string_pretty(&build_runtime_candidate_manifest(
        project_root,
        memory_root,
    )?)
    .map_err(|error| format!("failed to serialize runtime candidate manifest: {error}"))
}

pub fn proof_support_flags(proof: &ProofReport) -> Vec<String> {
    let mut flags = Vec::new();
    push_flag(
        &mut flags,
        "local_corpus_ingestion_support",
        proof.local_corpus_ingestion_support,
    );
    push_flag(
        &mut flags,
        "read_only_corpus_safety",
        proof.read_only_corpus_safety,
    );
    push_flag(
        &mut flags,
        "task_suggestion_support",
        proof.task_suggestion_support,
    );
    push_flag(
        &mut flags,
        "campaign_diagnostics_support",
        proof.campaign_diagnostics_support,
    );
    push_flag(
        &mut flags,
        "zero_yield_task_adjustment_support",
        proof.zero_yield_task_adjustment_support,
    );
    push_flag(
        &mut flags,
        "bounded_campaign_loop_support",
        proof.bounded_campaign_loop_support,
    );
    push_flag(
        &mut flags,
        "recombination_fallback_support",
        proof.recombination_fallback_support,
    );
    push_flag(
        &mut flags,
        "replay_review_support",
        proof.replay_review_support,
    );
    push_flag(
        &mut flags,
        "promotion_queue_support",
        proof.promotion_queue_support,
    );
    push_flag(
        &mut flags,
        "supervised_task_support",
        proof.supervised_task_support,
    );
    push_flag(
        &mut flags,
        "governance_runtime_support",
        proof.governance_runtime_support,
    );
    push_flag(
        &mut flags,
        "release_runtime_support",
        proof.release_runtime_support,
    );
    push_flag(
        &mut flags,
        "release_health_support",
        proof.release_health_support,
    );
    push_flag(
        &mut flags,
        "artifact_audit_support",
        proof.artifact_audit_support,
    );
    push_flag(
        &mut flags,
        "determinism_audit_support",
        proof.determinism_audit_support,
    );
    push_flag(
        &mut flags,
        "preflight_gate_v2_support",
        proof.preflight_gate_v2_support,
    );
    push_flag(
        &mut flags,
        "release_ledger_support",
        proof.release_ledger_support,
    );
    push_flag(
        &mut flags,
        "future_phase_registry_support",
        proof.future_phase_registry_support,
    );
    push_flag(
        &mut flags,
        "operator_runbook_support",
        proof.operator_runbook_support,
    );
    push_flag(
        &mut flags,
        "operations_runtime_support",
        proof.operations_runtime_support,
    );
    push_flag(&mut flags, "pr_package_support", proof.pr_package_support);
    push_flag(
        &mut flags,
        "external_patch_package_support",
        proof.external_patch_package_support,
    );
    push_flag(
        &mut flags,
        "self_review_package_support",
        proof.self_review_package_support,
    );
    push_flag(
        &mut flags,
        "operator_console_support",
        proof.operator_console_support,
    );
    push_flag(
        &mut flags,
        "capability_policy_support",
        proof.capability_policy_support,
    );
    push_flag(
        &mut flags,
        "trust_decision_support",
        proof.trust_decision_support,
    );
    push_flag(
        &mut flags,
        "evidence_bundle_support",
        proof.evidence_bundle_support,
    );
    push_flag(
        &mut flags,
        "workspace_snapshot_support",
        proof.workspace_snapshot_support,
    );
    push_flag(
        &mut flags,
        "recovery_manifest_support",
        proof.recovery_manifest_support,
    );
    push_flag(
        &mut flags,
        "preflight_gate_v3_support",
        proof.preflight_gate_v3_support,
    );
    push_flag(
        &mut flags,
        "trust_proof_report_support",
        proof.trust_proof_report_support,
    );
    push_flag(
        &mut flags,
        "runtime_candidate_support",
        proof.runtime_candidate_support,
    );
    push_flag(
        &mut flags,
        "runtime_validation_support",
        proof.runtime_validation_support,
    );
    push_flag(
        &mut flags,
        "runtime_service_metadata_support",
        proof.runtime_service_metadata_support,
    );
    push_flag(
        &mut flags,
        "stable_cli_contract_support",
        proof.stable_cli_contract_support,
    );
    push_flag(
        &mut flags,
        "final_rc_report_support",
        proof.final_rc_report_support,
    );
    flags
}

fn push_flag(flags: &mut Vec<String>, name: &str, enabled: bool) {
    if enabled {
        flags.push(name.to_string());
    }
}

fn git_stdout(project_root: &str, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(project_root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
