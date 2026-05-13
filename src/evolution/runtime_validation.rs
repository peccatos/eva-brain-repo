use std::path::Path;
use std::{fs, path::PathBuf};

use crate::contracts::{
    ArtifactAuditReport, CapabilityPolicy, DeterminismAuditReport, GovernanceStatus,
    PreflightGateV3Report, ProofReport, ReleaseHealthReport, RuntimeValidation, WorkspaceSnapshot,
};
use crate::evolution::{
    build_artifact_audit, build_capability_policy, build_determinism_audit,
    build_preflight_gate_v3, build_proof_report, build_release_candidate_state,
    build_release_health, build_workspace_snapshot, governance_status, load_metrics,
    load_or_refresh_promotion_queue, memory,
};

pub fn build_runtime_validation(
    project_root: &str,
    memory_root: &str,
) -> Result<RuntimeValidation, String> {
    let policy = build_capability_policy();
    let governance = governance_status(project_root, memory_root)?;
    let proof = build_proof_report(project_root, memory_root)?;
    let health = build_release_health(project_root, memory_root)?;
    let artifact = build_artifact_audit(project_root)?;
    let determinism = build_determinism_audit(project_root, memory_root)?;
    let gate_v3 = build_preflight_gate_v3(project_root, memory_root)?;
    let snapshot = build_workspace_snapshot(project_root, memory_root)?;
    let mut validation = evaluate_runtime_validation(
        memory::now_unix(),
        &policy,
        &governance,
        &proof,
        &health,
        &artifact,
        &determinism,
        &gate_v3,
        &snapshot,
    );
    apply_green_gate_details(project_root, memory_root, &mut validation)?;
    write_runtime_validation(memory_root, &validation)?;
    Ok(validation)
}

pub fn load_latest_runtime_validation(
    memory_root: &str,
) -> Result<Option<RuntimeValidation>, String> {
    let dir = Path::new(memory_root).join("runtime_validation");
    if !dir.exists() {
        return Ok(None);
    }
    let mut entries = Vec::new();
    for entry in fs::read_dir(&dir)
        .map_err(|error| format!("failed to read runtime validation dir: {error}"))?
    {
        let entry =
            entry.map_err(|error| format!("failed to read runtime validation entry: {error}"))?;
        let path = entry.path();
        if !path.extension().is_some_and(|ext| ext == "json") {
            continue;
        }
        let contents = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read runtime validation: {error}"))?;
        let validation: RuntimeValidation = serde_json::from_str(&contents)
            .map_err(|error| format!("failed to parse runtime validation: {error}"))?;
        entries.push((
            validation.generated_at,
            path,
            sanitize_runtime_validation(validation),
        ));
    }
    entries.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| file_label(&left.1).cmp(&file_label(&right.1)))
    });
    Ok(entries.pop().map(|(_, _, validation)| validation))
}

fn sanitize_runtime_validation(mut validation: RuntimeValidation) -> RuntimeValidation {
    validation.warnings.retain(|warning| {
        !validation
            .missing_green_conditions
            .iter()
            .any(|missing| missing == warning)
    });
    validation.warnings.sort();
    validation.warnings.dedup();
    validation.blockers.sort();
    validation.blockers.dedup();
    validation.green_conditions.sort();
    validation.green_conditions.dedup();
    validation.missing_green_conditions.sort();
    validation.missing_green_conditions.dedup();
    validation.status = if !validation.blockers.is_empty() {
        "blocked".to_string()
    } else if validation.missing_green_conditions.is_empty() && validation.warnings.is_empty() {
        "green".to_string()
    } else {
        "warn".to_string()
    };
    validation
}

pub fn load_or_build_runtime_validation(
    project_root: &str,
    memory_root: &str,
) -> Result<RuntimeValidation, String> {
    if let Some(validation) = load_latest_runtime_validation(memory_root)? {
        return Ok(validation);
    }
    build_runtime_validation(project_root, memory_root)
}

#[allow(clippy::too_many_arguments)]
pub fn evaluate_runtime_validation(
    generated_at: u64,
    policy: &CapabilityPolicy,
    governance: &GovernanceStatus,
    proof: &ProofReport,
    health: &ReleaseHealthReport,
    artifact: &ArtifactAuditReport,
    determinism: &DeterminismAuditReport,
    gate_v3: &PreflightGateV3Report,
    snapshot: &WorkspaceSnapshot,
) -> RuntimeValidation {
    let mut blockers = Vec::new();
    let mut warnings = Vec::new();
    let mut checks = Vec::new();

    if policy.auto_promote_allowed
        || policy.network_push_allowed
        || policy.merge_allowed
        || policy.external_repo_mutation_allowed
        || policy.self_apply_allowed
        || policy.source_mutation_without_approval_allowed
    {
        blockers.push("unsafe_capability_policy".to_string());
    }
    if !policy.metadata_generation_allowed || !policy.local_read_only_inspection_allowed {
        blockers.push("required_metadata_capabilities_missing".to_string());
    }
    if !policy.sandboxed_validation_allowed_when_isolated {
        blockers.push("isolated_sandbox_validation_disabled".to_string());
    }
    if artifact.should_fail_release || !artifact.sandbox_leaks.is_empty() {
        blockers.push("artifact_audit_failed".to_string());
    }
    if snapshot.sandbox_leak_count > 0 {
        blockers.push("sandbox_leaks_present".to_string());
    }
    if !determinism.full_source_content_warnings.is_empty() {
        blockers.push("full_source_content_detected".to_string());
    }
    if !determinism.deterministic_enough {
        blockers.push("determinism_audit_failed".to_string());
    }
    if proof.auto_promote || health.auto_promote || gate_v3.auto_promote {
        blockers.push("auto_promote_true_detected".to_string());
    }
    if !proof.operator_approval_required
        || !health.operator_approval_required
        || !governance.operator_approval_required
        || !gate_v3.operator_approval_required
    {
        blockers.push("operator_approval_disabled".to_string());
    }
    if gate_v3.status == "fail" {
        blockers.push("preflight_gate_v3_failed".to_string());
    }
    if governance.promotion_ready_approved_count == 0 {
        warnings.push("no_approved_release_candidate".to_string());
    }
    if gate_v3.status == "warn" {
        warnings.push("preflight_gate_v3_warn".to_string());
    }
    if snapshot.modified_count > 0 || snapshot.untracked_count > 0 {
        warnings.push("workspace_not_clean".to_string());
    }
    if health.health_grade == "yellow" {
        warnings.push("release_health_yellow".to_string());
    }

    checks.push(format!(
        "capability_policy:{}",
        if blockers
            .iter()
            .any(|item| item == "unsafe_capability_policy")
        {
            "fail"
        } else {
            "pass"
        }
    ));
    checks.push(format!(
        "governance:approved={} ready_approved={} approval_required={}",
        governance.approved_count,
        governance.promotion_ready_approved_count,
        governance.operator_approval_required
    ));
    checks.push(format!(
        "proof:support_flags={} auto_promote={} approval_required={}",
        proof_support_count(proof),
        proof.auto_promote,
        proof.operator_approval_required
    ));
    checks.push(format!(
        "release_health:{} score={}",
        health.health_grade, health.health_score
    ));
    checks.push(format!(
        "artifact_audit:fail={} sandbox_leaks={}",
        artifact.should_fail_release,
        artifact.sandbox_leaks.len()
    ));
    checks.push(format!(
        "determinism:deterministic_enough={} full_source_warnings={}",
        determinism.deterministic_enough,
        determinism.full_source_content_warnings.len()
    ));
    checks.push(format!("preflight_gate_v3:{}", gate_v3.status));
    checks.push(format!(
        "workspace_snapshot:modified={} untracked={} sandbox_leaks={}",
        snapshot.modified_count, snapshot.untracked_count, snapshot.sandbox_leak_count
    ));

    blockers.sort();
    blockers.dedup();
    warnings.sort();
    warnings.dedup();
    checks.sort();

    let status = if !blockers.is_empty() {
        "blocked"
    } else if !warnings.is_empty() {
        "warn"
    } else {
        "pass"
    }
    .to_string();
    let next_actions = if status == "blocked" {
        vec![
            "cargo run -- --artifact-audit".to_string(),
            "cargo run -- --preflight-gate-v3".to_string(),
            "cargo run -- --trust-proof-report".to_string(),
        ]
    } else if status == "warn" {
        vec![
            "cargo run -- --promotion-ready-approved".to_string(),
            "cargo run -- --runtime-candidate".to_string(),
            "cargo run -- --final-rc-report".to_string(),
        ]
    } else {
        vec![
            "cargo run -- --final-rc-report".to_string(),
            "cargo run -- --operator-console".to_string(),
        ]
    };

    RuntimeValidation {
        validation_id: format!("runtime-validation-{generated_at}"),
        generated_at,
        status,
        blockers,
        warnings,
        checks,
        next_actions,
        green_conditions: Vec::new(),
        missing_green_conditions: Vec::new(),
        approved_release_candidate: None,
        release_bundle: None,
        metrics_summary: String::new(),
        candidate_queue_summary: String::new(),
        sandbox_state: String::new(),
        auto_promote: false,
        operator_approval_required: true,
    }
}

fn apply_green_gate_details(
    project_root: &str,
    memory_root: &str,
    validation: &mut RuntimeValidation,
) -> Result<(), String> {
    let release_candidate = build_release_candidate_state(project_root, memory_root)?;
    let metrics = load_metrics(memory_root)?;
    let queue = load_or_refresh_promotion_queue(project_root, memory_root)?;
    let mut green_conditions = Vec::new();
    let mut missing = Vec::new();

    check_condition(
        &mut green_conditions,
        &mut missing,
        release_candidate.approved_release_candidate.is_some(),
        "approved_release_candidate",
    );
    check_condition(
        &mut green_conditions,
        &mut missing,
        release_candidate.release_bundle_exists,
        "release_bundle_exists",
    );
    check_condition(
        &mut green_conditions,
        &mut missing,
        release_candidate.preflight_gate_v3 == "pass",
        "preflight_gate_v3_pass",
    );
    check_condition(
        &mut green_conditions,
        &mut missing,
        release_candidate.release_health == "green",
        "release_health_green",
    );
    check_condition(
        &mut green_conditions,
        &mut missing,
        validation.sandbox_state != "leaked",
        "sandbox_leaks_zero",
    );
    check_condition(
        &mut green_conditions,
        &mut missing,
        metrics.failed_runs
            == metrics.real_execution_failed_runs
                + metrics.cargo_gate_failed_runs
                + metrics.replay_failed_runs,
        "metrics_semantics_correct",
    );
    check_condition(
        &mut green_conditions,
        &mut missing,
        queue.summary.ready_candidates > 0 || release_candidate.operator_approved,
        "candidate_queue_has_ready_or_approved_candidate",
    );
    check_condition(
        &mut green_conditions,
        &mut missing,
        validation.blockers.is_empty(),
        "no_critical_blockers",
    );
    check_condition(
        &mut green_conditions,
        &mut missing,
        release_candidate.operator_approval_required,
        "operator_approval_required",
    );
    check_condition(
        &mut green_conditions,
        &mut missing,
        release_candidate.operator_approved,
        "operator_approved",
    );
    let replay_ok_for_approved = release_candidate
        .approved_release_candidate
        .as_ref()
        .and_then(|run_id| queue.items.iter().find(|item| &item.run_id == run_id))
        .map(|item| item.replay_status == "ok" || item.replay_status == "passed")
        .unwrap_or(false);
    check_condition(
        &mut green_conditions,
        &mut missing,
        replay_ok_for_approved,
        "replay_status_for_approved_candidate",
    );

    validation.approved_release_candidate = release_candidate.approved_release_candidate;
    validation.release_bundle = release_candidate.latest_release_bundle;
    validation.metrics_summary = format!(
        "total_runs={} passed={} failed={} safety_rejected={} duplicate_rejected={}",
        metrics.total_runs,
        metrics.passed_runs,
        metrics.failed_runs,
        metrics.safety_rejected_runs,
        metrics.duplicate_rejected_runs
    );
    validation.candidate_queue_summary = format!(
        "candidate_count={} ready={} blocked={} quarantined={} legacy={} duplicate={} unreplayable={} already_promoted={}",
        queue.summary.candidate_count,
        queue.summary.ready_candidates,
        queue.items.len().saturating_sub(queue.summary.ready_candidates),
        queue.summary.quarantined_candidates,
        queue.summary.legacy_candidates,
        queue.summary.duplicate_candidates,
        queue.summary.unreplayable_candidates,
        queue.summary.already_promoted_candidates
    );
    validation.sandbox_state = if validation
        .blockers
        .iter()
        .any(|blocker| blocker.contains("sandbox"))
    {
        "leaked".to_string()
    } else {
        "clean".to_string()
    };
    validation.green_conditions = green_conditions;
    validation.missing_green_conditions = missing;
    validation.warnings.sort();
    validation.warnings.dedup();
    validation.status = if !validation.blockers.is_empty() {
        "blocked".to_string()
    } else if validation.missing_green_conditions.is_empty() {
        "green".to_string()
    } else {
        "warn".to_string()
    };
    Ok(())
}

fn check_condition(
    green_conditions: &mut Vec<String>,
    missing: &mut Vec<String>,
    ok: bool,
    label: &str,
) {
    if ok {
        green_conditions.push(label.to_string());
    } else {
        missing.push(label.to_string());
    }
}

pub fn print_runtime_validation(project_root: &str, memory_root: &str) -> Result<String, String> {
    serde_json::to_string_pretty(&load_or_build_runtime_validation(
        project_root,
        memory_root,
    )?)
    .map_err(|error| format!("failed to serialize runtime validation: {error}"))
}

fn write_runtime_validation(
    memory_root: &str,
    validation: &RuntimeValidation,
) -> Result<(), String> {
    memory::write_json(
        Path::new(memory_root)
            .join("runtime_validation")
            .join(format!("{}.json", validation.validation_id)),
        validation,
    )
}

fn file_label(path: &PathBuf) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string()
}

fn proof_support_count(proof: &ProofReport) -> usize {
    [
        proof.local_corpus_ingestion_support,
        proof.read_only_corpus_safety,
        proof.task_suggestion_support,
        proof.campaign_diagnostics_support,
        proof.zero_yield_task_adjustment_support,
        proof.bounded_campaign_loop_support,
        proof.recombination_fallback_support,
        proof.replay_review_support,
        proof.promotion_queue_support,
        proof.supervised_task_support,
        proof.governance_runtime_support,
        proof.release_runtime_support,
        proof.release_health_support,
        proof.artifact_audit_support,
        proof.determinism_audit_support,
        proof.preflight_gate_v2_support,
        proof.release_ledger_support,
        proof.future_phase_registry_support,
        proof.operator_runbook_support,
        proof.operations_runtime_support,
        proof.pr_package_support,
        proof.external_patch_package_support,
        proof.self_review_package_support,
        proof.operator_console_support,
        proof.capability_policy_support,
        proof.trust_decision_support,
        proof.evidence_bundle_support,
        proof.workspace_snapshot_support,
        proof.recovery_manifest_support,
        proof.preflight_gate_v3_support,
        proof.trust_proof_report_support,
        proof.runtime_candidate_support,
        proof.runtime_validation_support,
        proof.runtime_service_metadata_support,
        proof.stable_cli_contract_support,
        proof.final_rc_report_support,
    ]
    .into_iter()
    .filter(|enabled| *enabled)
    .count()
}
