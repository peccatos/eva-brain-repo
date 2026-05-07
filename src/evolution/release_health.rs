use std::path::Path;

use crate::contracts::ReleaseHealthReport;
use crate::evolution::{
    build_artifact_audit, governance_status, latest_release_id, load_or_refresh_promotion_queue,
    memory, release_count,
};

pub fn build_release_health(
    project_root: &str,
    memory_root: &str,
) -> Result<ReleaseHealthReport, String> {
    let queue = load_or_refresh_promotion_queue(project_root, memory_root)?;
    let governance = governance_status(project_root, memory_root)?;
    let artifact = build_artifact_audit(project_root)?;
    let candidate_count = queue.items.len();
    let ready_count = queue
        .items
        .iter()
        .filter(|item| item.lifecycle_state == "ready")
        .count();
    let blocked_count = candidate_count.saturating_sub(ready_count);
    let replay_passed_candidates = queue
        .items
        .iter()
        .filter(|item| item.replay_status == "ok" && item.mutation_class == "useful")
        .count();
    let promoted_candidates = queue
        .items
        .iter()
        .filter(|item| item.lifecycle_state == "already_promoted")
        .count();
    let release_count = release_count(memory_root)?;
    let latest_release_id = latest_release_id(memory_root)?;
    let governance_ready = governance.operator_approval_required && !governance.auto_promote;
    let proof_ready = Path::new(memory_root)
        .join("proof")
        .join("eva_proof.json")
        .exists();
    let preflight_ready = Path::new(memory_root)
        .join("releases")
        .join("preflight")
        .exists();
    let sandbox_leaks_detected = !artifact.sandbox_leaks.is_empty();
    let mut blockers = Vec::new();
    if governance.auto_promote {
        blockers.push("auto_promote_true".to_string());
    }
    if sandbox_leaks_detected {
        blockers.push("sandbox_leaks_detected".to_string());
    }
    if !artifact.tracked_runtime_artifacts.is_empty() {
        blockers.push("tracked_runtime_artifacts".to_string());
    }
    if !governance_ready {
        blockers.push("governance_not_ready".to_string());
    }

    let mut score = 100_u32;
    score = score.saturating_sub((blocked_count as u32).saturating_mul(3).min(30));
    if candidate_count == 0 {
        score = score.saturating_sub(20);
    }
    if release_count == 0 {
        score = score.saturating_sub(10);
    }
    if !proof_ready {
        score = score.saturating_sub(10);
    }
    if !preflight_ready {
        score = score.saturating_sub(5);
    }
    if !governance_ready {
        score = score.saturating_sub(30);
    }
    if sandbox_leaks_detected
        || governance.auto_promote
        || !artifact.tracked_runtime_artifacts.is_empty()
    {
        score = score.min(40);
    }
    let health_grade = if sandbox_leaks_detected || governance.auto_promote {
        "red"
    } else if score >= 80 && ready_count > 0 {
        "green"
    } else if score >= 50 {
        "yellow"
    } else {
        "red"
    }
    .to_string();

    let mut recommendations_ru = Vec::new();
    if candidate_count == 0 {
        recommendations_ru
            .push("Нет release-кандидатов: это предупреждение, не авария.".to_string());
    }
    if ready_count == 0 {
        recommendations_ru
            .push("Подготовить replay-ok useful кандидата и approval перед release.".to_string());
    }
    if sandbox_leaks_detected {
        recommendations_ru.push("Очистить sandboxes перед release gate.".to_string());
    }
    if recommendations_ru.is_empty() {
        recommendations_ru.push("Release runtime готов к metadata-only проверкам.".to_string());
    }

    Ok(ReleaseHealthReport {
        generated_at: memory::now_unix(),
        release_runtime_support: true,
        latest_release_id,
        release_count,
        candidate_count,
        approved_count: governance.approved_count,
        ready_count,
        blocked_count,
        replay_passed_candidates,
        promoted_candidates,
        governance_ready,
        proof_ready,
        preflight_ready,
        sandbox_leaks_detected,
        auto_promote: false,
        operator_approval_required: true,
        health_score: score,
        health_grade,
        blockers,
        recommendations_ru,
    })
}

pub fn print_release_health(project_root: &str, memory_root: &str) -> Result<String, String> {
    let health = build_release_health(project_root, memory_root)?;
    Ok(format!(
        "release_health: grade={} score={} releases={} latest={} candidates={} ready={} blocked={} auto_promote={} approval_required={} blockers={}",
        health.health_grade,
        health.health_score,
        health.release_count,
        health.latest_release_id.as_deref().unwrap_or("none"),
        health.candidate_count,
        health.ready_count,
        health.blocked_count,
        health.auto_promote,
        health.operator_approval_required,
        if health.blockers.is_empty() { "none".to_string() } else { health.blockers.join(",") }
    ))
}

pub fn print_release_health_json(project_root: &str, memory_root: &str) -> Result<String, String> {
    serde_json::to_string_pretty(&build_release_health(project_root, memory_root)?)
        .map_err(|error| format!("failed to serialize release health: {error}"))
}
