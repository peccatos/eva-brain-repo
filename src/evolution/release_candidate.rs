use std::fs;
use std::path::Path;

use crate::contracts::{ReleaseCandidateApprovalReport, ReleaseCandidateState};
use crate::evolution::{
    approve_candidate, build_evidence_bundle, build_preflight_gate_v3, build_release_health,
    candidate_lifecycle, governance_status, latest_decisions, latest_release_id, memory,
    refresh_promotion_queue,
};

pub fn build_release_candidate_state(
    project_root: &str,
    memory_root: &str,
) -> Result<ReleaseCandidateState, String> {
    let _ = refresh_promotion_queue(project_root, memory_root);
    let governance = governance_status(project_root, memory_root)?;
    let gate = build_preflight_gate_v3(project_root, memory_root)?;
    let health = build_release_health(project_root, memory_root)?;
    let latest_release = latest_release_id(memory_root)?;
    let approved = governance.promotion_ready_approved_count > 0;
    let mut blockers = Vec::new();
    let mut warnings = Vec::new();
    if !approved {
        warnings.push("no_approved_release_candidate".to_string());
    }
    if latest_release.is_none() {
        warnings.push("no_release_bundle".to_string());
    }
    if gate.status == "fail" {
        blockers.push("preflight_gate_v3_failed".to_string());
    } else if gate.status == "warn" {
        warnings.push("preflight_gate_v3_warn".to_string());
    }
    if health.health_grade == "red" {
        blockers.push("release_health_red".to_string());
    } else if health.health_grade == "yellow" {
        warnings.push("release_health_yellow".to_string());
    }

    let approved_run = latest_decisions(memory_root)?
        .into_iter()
        .filter(|record| record.decision == "approve")
        .map(|record| record.run_id)
        .min();

    Ok(ReleaseCandidateState {
        approved_release_candidate: approved_run,
        approved_count: governance.promotion_ready_approved_count as u64,
        release_bundle_exists: latest_release.is_some(),
        latest_release_bundle: latest_release,
        operator_approval_required: true,
        operator_approved: approved,
        preflight_gate_v3: gate.status,
        release_health: health.health_grade,
        blockers,
        warnings,
    })
}

pub fn approve_release_candidate(
    project_root: &str,
    memory_root: &str,
    run_id: &str,
) -> Result<ReleaseCandidateApprovalReport, String> {
    let item = candidate_lifecycle(project_root, memory_root, run_id)?;
    let mut blockers = Vec::new();
    let reason = if item.candidate_state != crate::contracts::CandidateState::Ready {
        format!(
            "candidate_state={}",
            format!("{:?}", item.candidate_state).to_ascii_lowercase()
        )
    } else if item.replay_status != "ok" {
        format!("replay_status={}", item.replay_status)
    } else if item.cargo_test_ok != Some(true) {
        "cargo_test_ok=false".to_string()
    } else if item.cargo_run_ok != Some(true) {
        "cargo_run_ok=false".to_string()
    } else if !item.promotion_blockers.is_empty() {
        "promotion_gate_blocked".to_string()
    } else {
        "unknown".to_string()
    };
    if item.candidate_state != crate::contracts::CandidateState::Ready {
        blockers.push(format!(
            "candidate_state={}",
            format!("{:?}", item.candidate_state).to_ascii_lowercase()
        ));
    }
    if item.replay_status != "ok" {
        blockers.push("replay_not_ok".to_string());
    }
    if item.cargo_test_ok != Some(true) {
        blockers.push("cargo_test_not_ok".to_string());
    }
    if item.cargo_run_ok != Some(true) {
        blockers.push("cargo_run_not_ok".to_string());
    }
    if !item.promotion_blockers.is_empty() {
        blockers.extend(item.promotion_blockers.clone());
    }
    if !blockers.is_empty() {
        blockers.sort();
        blockers.dedup();
        return Err(format!(
            "release approval refused\nrun_id={run_id}\nreason={reason}\nblockers={}",
            blockers.join(",")
        ));
    }

    let approval = approve_candidate(
        project_root,
        memory_root,
        run_id,
        "operator approved release candidate",
    )?;
    let evidence = build_evidence_bundle(project_root, memory_root)?;
    let base = Path::new(memory_root)
        .join("release_candidates")
        .join(format!("rc-{run_id}"));
    fs::create_dir_all(&base)
        .map_err(|error| format!("failed to create release candidate dir: {error}"))?;
    let release_candidate_path = base.join("release_candidate.json");
    let evidence_bundle_path = base.join("evidence_bundle.json");
    let validation_report_path = base.join("validation_report.md");
    let replay_report_path = Path::new(memory_root)
        .join("replays")
        .join(format!("{run_id}.json"));
    let candidate_summary_path = Path::new(memory_root)
        .join("candidates")
        .join(format!("{run_id}.summary.json"));
    let metrics_snapshot_path = Path::new(memory_root).join("metrics.json");

    let report = ReleaseCandidateApprovalReport {
        run_id: run_id.to_string(),
        candidate_state: format!("{:?}", item.candidate_state),
        replay_status: item.replay_status.clone(),
        cargo_test_ok: item.cargo_test_ok,
        cargo_run_ok: item.cargo_run_ok,
        operator_approved: approval.decision == "approve",
        evidence_bundle_path: evidence_bundle_path.display().to_string(),
        validation_report_path: validation_report_path.display().to_string(),
        release_candidate_path: release_candidate_path.display().to_string(),
        blockers: Vec::new(),
        warnings: Vec::new(),
        generated_at: memory::now_unix(),
    };
    memory::write_json(&release_candidate_path, &report)?;
    memory::write_json(&evidence_bundle_path, &evidence)?;
    let validation_report = format!(
        "# EVA Release Candidate\n\nrun_id={run_id}\ncandidate_state={}\nreplay_status={}\ncargo_test_ok={:?}\ncargo_run_ok={:?}\noperator_approved={}\nreplay_report={}\nmetrics_snapshot={}\ncandidate_summary={}\nauto_promote=false\noperator_approval_required=true\n",
        report.candidate_state,
        report.replay_status,
        report.cargo_test_ok,
        report.cargo_run_ok,
        report.operator_approved,
        replay_report_path.display(),
        metrics_snapshot_path.display(),
        candidate_summary_path.display()
    );
    fs::write(&validation_report_path, validation_report)
        .map_err(|error| format!("failed to write release candidate report: {error}"))?;
    Ok(report)
}

pub fn print_release_approve(
    project_root: &str,
    memory_root: &str,
    run_id: &str,
) -> Result<String, String> {
    serde_json::to_string_pretty(&approve_release_candidate(
        project_root,
        memory_root,
        run_id,
    )?)
    .map_err(|error| format!("failed to serialize release candidate approval: {error}"))
}
