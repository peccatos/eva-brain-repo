use std::fs;
use std::path::Path;

use crate::contracts::EvolutionLogEntry;
use crate::contracts::ProofReport;
use crate::evolution::{
    build_preflight_gate, build_release_health, governance_status, latest_proof_snapshot_id,
    latest_release_id, latest_supervised_run_id, load_or_refresh_promotion_queue, memory,
    print_artifact_audit, print_operator_runbook, print_proof_snapshot, print_release_status,
    refresh_metrics, release_count, release_ledger_count,
};

pub fn build_proof_report(project_root: &str, memory_root: &str) -> Result<ProofReport, String> {
    let _ = refresh_metrics(memory_root);
    let queue = load_or_refresh_promotion_queue(project_root, memory_root)?;
    let governance = governance_status(project_root, memory_root)?;
    let summaries = memory::list_candidate_summaries(memory_root)?;
    let total_runs = total_run_count(memory_root)?;
    let promoted_candidates = promoted_count(memory_root)?;
    let replay_passed_candidates = queue
        .items
        .iter()
        .filter(|item| item.replay_status == "ok")
        .count();
    let ready_candidates = queue
        .items
        .iter()
        .filter(|item| item.lifecycle_state == "ready")
        .count();
    let blocked_candidates = queue.items.len().saturating_sub(ready_candidates);
    let proof = ProofReport {
        generated_at: derived_generated_at(memory_root, queue.generated_at)?,
        local_corpus_ingestion_support: true,
        read_only_corpus_safety: true,
        task_suggestion_support: true,
        campaign_diagnostics_support: true,
        zero_yield_task_adjustment_support: true,
        bounded_campaign_loop_support: true,
        recombination_fallback_support: true,
        replay_review_support: true,
        promotion_queue_support: true,
        supervised_task_support: true,
        governance_runtime_support: true,
        release_runtime_support: true,
        release_health_support: true,
        artifact_audit_support: true,
        determinism_audit_support: true,
        preflight_gate_v2_support: true,
        release_ledger_support: true,
        future_phase_registry_support: true,
        operator_runbook_support: true,
        auto_promote: false,
        operator_approval_required: true,
        forbidden_target_preservation: true,
        total_runs,
        candidate_count: summaries.len(),
        replay_passed_candidates,
        promoted_candidates,
        ready_candidates,
        blocked_candidates,
        approved_count: governance.approved_count,
        rejected_count: governance.rejected_count,
        deferred_count: governance.deferred_count,
        release_count: release_count(memory_root)?,
        release_ledger_count: release_ledger_count(memory_root)?,
        latest_release_id: latest_release_id(memory_root)?,
        latest_bounded_run_id: latest_id_from_dir(memory_root, "bounded_runs")?,
        latest_supervised_run_id: latest_supervised_run_id(memory_root)?,
    };
    write_proof_report(memory_root, &proof)?;
    Ok(proof)
}

fn derived_generated_at(memory_root: &str, queue_generated_at: u64) -> Result<u64, String> {
    Ok(queue_generated_at
        .max(latest_artifact_timestamp(
            memory_root,
            "proof",
            "eva_proof.json",
        )?)
        .max(latest_dir_timestamp(memory_root, "bounded_runs")?)
        .max(latest_dir_timestamp(memory_root, "supervised_runs")?))
}

pub fn print_eva_status(project_root: &str, memory_root: &str) -> Result<String, String> {
    let proof = build_proof_report(project_root, memory_root)?;
    Ok(format!(
        "eva_status: candidates={} ready={} blocked={} replay_passed={} promoted={} approved={} rejected={} deferred={} releases={} release_latest={} bounded_latest={} supervised_latest={} proof_snapshot_latest={} auto_promote={}",
        proof.candidate_count,
        proof.ready_candidates,
        proof.blocked_candidates,
        proof.replay_passed_candidates,
        proof.promoted_candidates,
        proof.approved_count,
        proof.rejected_count,
        proof.deferred_count,
        proof.release_count,
        proof.latest_release_id.as_deref().unwrap_or("none"),
        proof.latest_bounded_run_id.as_deref().unwrap_or("none"),
        proof.latest_supervised_run_id.as_deref().unwrap_or("none"),
        latest_proof_snapshot_id(memory_root)?.as_deref().unwrap_or("none"),
        proof.auto_promote
    ))
}

pub fn print_proof_report(project_root: &str, memory_root: &str) -> Result<String, String> {
    let proof = build_proof_report(project_root, memory_root)?;
    Ok(render_proof_markdown(&proof))
}

pub fn print_proof_json(project_root: &str, memory_root: &str) -> Result<String, String> {
    let proof = build_proof_report(project_root, memory_root)?;
    serde_json::to_string_pretty(&proof)
        .map_err(|error| format!("failed to serialize proof json: {error}"))
}

pub fn run_demo(project_root: &str, memory_root: &str) -> Result<String, String> {
    let _ = load_or_refresh_promotion_queue(project_root, memory_root)?;
    let governance = governance_status(project_root, memory_root)?;
    let status = print_eva_status(project_root, memory_root)?;
    let report = print_proof_report(project_root, memory_root)?;
    let snapshot = print_proof_snapshot(project_root, memory_root)?;
    let release_status = print_release_status(memory_root)?;
    let health = build_release_health(project_root, memory_root)?;
    let gate = build_preflight_gate(project_root, memory_root)?;
    let artifact = print_artifact_audit(project_root)?;
    let runbook = print_operator_runbook(project_root, memory_root)?;
    Ok(format!(
        "{status}\n\ngovernance_status: approved={} rejected={} deferred={} ready_approved={} auto_promote={}\n\nrelease_status: {}\nrelease_health: grade={} score={}\npreflight_gate: status={}\n\n{}\n\n{}\n\n{}\n\n{}",
        governance.approved_count,
        governance.rejected_count,
        governance.deferred_count,
        governance.promotion_ready_approved_count,
        governance.auto_promote,
        release_status,
        health.health_grade,
        health.health_score,
        gate.gate_status,
        artifact,
        runbook,
        report,
        snapshot
    ))
}

fn total_run_count(memory_root: &str) -> Result<u64, String> {
    let path = Path::new(memory_root).join("evolution.jsonl");
    if !path.exists() {
        return Ok(0);
    }
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read evolution log: {error}"))?;
    Ok(contents
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count() as u64)
}

fn promoted_count(memory_root: &str) -> Result<usize, String> {
    let path = Path::new(memory_root).join("evolution.jsonl");
    if !path.exists() {
        return Ok(0);
    }
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read evolution log: {error}"))?;
    Ok(contents
        .lines()
        .filter_map(|line| serde_json::from_str::<EvolutionLogEntry>(line).ok())
        .filter(|entry| entry.retained_in_core)
        .count())
}

fn latest_id_from_dir(memory_root: &str, dir_name: &str) -> Result<Option<String>, String> {
    let dir = Path::new(memory_root).join(dir_name);
    if !dir.exists() {
        return Ok(None);
    }
    let mut entries = fs::read_dir(dir)
        .map_err(|error| format!("failed to read {dir_name}: {error}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .filter_map(|path| {
            let modified = fs::metadata(&path).ok()?.modified().ok()?;
            let modified = modified
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?
                .as_secs();
            let id = path.file_stem()?.to_str()?.to_string();
            Some((modified, id))
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));
    Ok(entries.into_iter().next().map(|(_, id)| id))
}

fn latest_dir_timestamp(memory_root: &str, dir_name: &str) -> Result<u64, String> {
    let dir = Path::new(memory_root).join(dir_name);
    if !dir.exists() {
        return Ok(0);
    }
    let mut newest = 0_u64;
    for entry in fs::read_dir(dir).map_err(|error| format!("failed to read {dir_name}: {error}"))? {
        let entry = entry.map_err(|error| format!("failed to read {dir_name} entry: {error}"))?;
        let modified = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        newest = newest.max(modified);
    }
    Ok(newest)
}

fn latest_artifact_timestamp(
    memory_root: &str,
    dir_name: &str,
    file_name: &str,
) -> Result<u64, String> {
    let path = Path::new(memory_root).join(dir_name).join(file_name);
    if !path.exists() {
        return Ok(0);
    }
    Ok(fs::metadata(path)
        .map_err(|error| format!("failed to read proof artifact metadata: {error}"))?
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or(0))
}

fn write_proof_report(memory_root: &str, proof: &ProofReport) -> Result<(), String> {
    let dir = Path::new(memory_root).join("proof");
    fs::create_dir_all(&dir).map_err(|error| format!("failed to create proof dir: {error}"))?;
    memory::write_json(dir.join("eva_proof.json"), proof)?;
    fs::write(dir.join("eva_proof.ru.md"), render_proof_markdown(proof))
        .map_err(|error| format!("failed to write proof markdown: {error}"))
}

fn render_proof_markdown(proof: &ProofReport) -> String {
    format!(
        "# EVA Proof Report\n\nlocal_corpus_ingestion_support={}\nread_only_corpus_safety={}\ntask_suggestion_support={}\ncampaign_diagnostics_support={}\nzero_yield_task_adjustment_support={}\nbounded_campaign_loop_support={}\nrecombination_fallback_support={}\nreplay_review_support={}\npromotion_queue_support={}\nsupervised_task_support={}\ngovernance_runtime_support={}\nrelease_runtime_support={}\nrelease_health_support={}\nartifact_audit_support={}\ndeterminism_audit_support={}\npreflight_gate_v2_support={}\nrelease_ledger_support={}\nfuture_phase_registry_support={}\noperator_runbook_support={}\nauto_promote={}\noperator_approval_required={}\nforbidden_target_preservation={}\n\ntotal_runs={}\ncandidate_count={}\nreplay_passed_candidates={}\npromoted_candidates={}\nready_candidates={}\nblocked_candidates={}\napproved_count={}\nrejected_count={}\ndeferred_count={}\nrelease_count={}\nrelease_ledger_count={}\nlatest_release_id={}\nlatest_bounded_run_id={}\nlatest_supervised_run_id={}\n",
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
        proof.auto_promote,
        proof.operator_approval_required,
        proof.forbidden_target_preservation,
        proof.total_runs,
        proof.candidate_count,
        proof.replay_passed_candidates,
        proof.promoted_candidates,
        proof.ready_candidates,
        proof.blocked_candidates,
        proof.approved_count,
        proof.rejected_count,
        proof.deferred_count,
        proof.release_count,
        proof.release_ledger_count,
        proof.latest_release_id.as_deref().unwrap_or("none"),
        proof.latest_bounded_run_id.as_deref().unwrap_or("none"),
        proof.latest_supervised_run_id.as_deref().unwrap_or("none"),
    )
}
