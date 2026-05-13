use std::fs;
use std::path::Path;

use crate::contracts::{CandidateQueueSummary, CandidateState, PromotionQueue, PromotionQueueItem};
use crate::evolution::{
    classify_mutation_kind_label, load_report_json, memory, mutation_class_label, refresh_metrics,
    refresh_report,
};
use crate::promotion::review::review_candidate;

pub fn refresh_promotion_queue(
    project_root: &str,
    memory_root: &str,
) -> Result<PromotionQueue, String> {
    let _ = refresh_metrics(memory_root);
    let summaries = memory::list_candidate_summaries(memory_root)?;
    let mut items = summaries
        .iter()
        .map(|summary| build_queue_item(project_root, memory_root, summary))
        .collect::<Result<Vec<_>, _>>()?;
    items.sort_by(|left, right| left.run_id.cmp(&right.run_id));
    let queue = PromotionQueue {
        summary: summarize_queue(&items),
        items,
        generated_at: memory::now_unix(),
    };
    write_promotion_queue(memory_root, &queue)?;
    Ok(queue)
}

pub fn load_promotion_queue(memory_root: &str) -> Result<PromotionQueue, String> {
    let path = Path::new(memory_root).join("promotion_queue.json");
    if !path.exists() {
        return Ok(PromotionQueue::default());
    }
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read promotion queue: {error}"))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("failed to parse promotion queue: {error}"))
}

pub fn load_or_refresh_promotion_queue(
    project_root: &str,
    memory_root: &str,
) -> Result<PromotionQueue, String> {
    let queue = load_promotion_queue(memory_root)?;
    if queue.items.is_empty() {
        return refresh_promotion_queue(project_root, memory_root);
    }
    Ok(queue)
}

pub fn print_promotion_queue(project_root: &str, memory_root: &str) -> Result<String, String> {
    let queue = load_or_refresh_promotion_queue(project_root, memory_root)?;
    let markdown_path = Path::new(memory_root).join("promotion_queue.ru.md");
    if markdown_path.exists() {
        return fs::read_to_string(markdown_path)
            .map_err(|error| format!("failed to read promotion queue markdown: {error}"));
    }
    Ok(render_promotion_queue_markdown(&queue))
}

pub fn promotion_ready_items(
    project_root: &str,
    memory_root: &str,
) -> Result<Vec<PromotionQueueItem>, String> {
    let mut items = load_or_refresh_promotion_queue(project_root, memory_root)?
        .items
        .into_iter()
        .filter(|item| item.lifecycle_state == "ready")
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.run_id.cmp(&right.run_id));
    Ok(items)
}

pub fn promotion_blocked_items(
    project_root: &str,
    memory_root: &str,
) -> Result<Vec<PromotionQueueItem>, String> {
    let mut items = load_or_refresh_promotion_queue(project_root, memory_root)?
        .items
        .into_iter()
        .filter(|item| item.lifecycle_state != "ready")
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.run_id.cmp(&right.run_id));
    Ok(items)
}

pub fn candidate_lifecycle(
    project_root: &str,
    memory_root: &str,
    run_id: &str,
) -> Result<PromotionQueueItem, String> {
    let queue = load_or_refresh_promotion_queue(project_root, memory_root)?;
    queue
        .items
        .into_iter()
        .find(|item| item.run_id == run_id)
        .ok_or_else(|| format!("candidate lifecycle not found for {run_id}"))
}

fn build_queue_item(
    project_root: &str,
    memory_root: &str,
    summary: &memory::CandidateSummary,
) -> Result<PromotionQueueItem, String> {
    let report_path = Path::new(memory_root)
        .join("reports")
        .join(format!("{}.ru.md", summary.run_id));
    let report_json_path = Path::new(memory_root)
        .join("reports")
        .join(format!("{}.report.json", summary.run_id));
    let report_exists = report_path.exists() && report_json_path.exists();
    if !report_exists {
        let _ = refresh_report(memory_root, &summary.run_id);
    }
    let rebuilt_report_exists = report_path.exists() && report_json_path.exists();
    let report = load_report_json(memory_root, &summary.run_id).ok();
    let mutation_class = report
        .as_ref()
        .map(|value| value.mutation_class.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            mutation_class_label(classify_mutation_kind_label(
                &summary.mutation_kind,
                summary.useful_change,
            ))
            .to_string()
        });

    let mut review = review_candidate(project_root, memory_root, &summary.run_id).ok();
    if review.is_none() && rebuilt_report_exists {
        review = review_candidate(project_root, memory_root, &summary.run_id).ok();
    }
    let (replay_status, promotion_blockers, promotion_allowed) = if let Some(review) = &review {
        (
            review.replay_status.clone(),
            review.promotion_blockers.clone(),
            review.promotion_allowed,
        )
    } else {
        (
            "unknown".to_string(),
            vec!["review_unavailable".to_string()],
            false,
        )
    };

    let (lifecycle_state, promotion_state, reason_ru) = classify_lifecycle(
        summary,
        &mutation_class,
        &replay_status,
        &promotion_blockers,
        promotion_allowed,
        rebuilt_report_exists,
    );
    let (candidate_state, candidate_state_reason) = classify_candidate_state(
        summary,
        &mutation_class,
        &replay_status,
        &promotion_blockers,
        rebuilt_report_exists,
    );

    Ok(PromotionQueueItem {
        run_id: summary.run_id.clone(),
        mutation_kind: summary.mutation_kind.clone(),
        mutation_class,
        target_file: summary.target_file.clone(),
        score: summary.score,
        risk: summary.risk,
        replay_status,
        promotion_state,
        promotion_allowed,
        promotion_blockers,
        report_path: report_path.display().to_string(),
        lifecycle_state,
        candidate_state,
        candidate_state_reason,
        cargo_test_ok: Some(summary.cargo_test_ok),
        cargo_run_ok: Some(summary.cargo_run_ok),
        duplicate_rejected: summary.duplicate_rejected,
        promoted: summary.status == crate::contracts::EvolutionStatus::Promoted,
        reason_ru,
        updated_at: memory::now_unix(),
    })
}

fn classify_candidate_state(
    summary: &memory::CandidateSummary,
    mutation_class: &str,
    replay_status: &str,
    promotion_blockers: &[String],
    report_exists: bool,
) -> (CandidateState, String) {
    if summary.duplicate_rejected {
        return (
            CandidateState::Duplicate,
            "duplicate safety rejection".to_string(),
        );
    }
    if !report_exists {
        return (
            CandidateState::Stale,
            "candidate report missing or not rebuildable".to_string(),
        );
    }
    if promotion_blockers.iter().any(|blocker| {
        matches!(
            blocker.as_str(),
            "candidate_missing" | "mutation_missing" | "report_missing"
        )
    }) {
        return (
            CandidateState::Blocked,
            "required candidate evidence is missing".to_string(),
        );
    }
    if mutation_class == "legacy" || mutation_class.is_empty() {
        return (
            CandidateState::Legacy,
            "legacy candidate cannot be replayed with full trust".to_string(),
        );
    }
    if mutation_class == "unsafe" || mutation_class == "cosmetic" {
        return (
            CandidateState::Quarantined,
            format!("{mutation_class} mutation is isolated from promotion"),
        );
    }
    if promotion_blockers
        .iter()
        .any(|blocker| blocker == "already_promoted")
    {
        return (
            CandidateState::AlreadyPromoted,
            "candidate already promoted".to_string(),
        );
    }
    if replay_status != "ok" && replay_status != "not_run" {
        return (
            CandidateState::Unreplayable,
            format!("replay status is {replay_status}"),
        );
    }
    if !summary.cargo_test_ok || !summary.cargo_run_ok {
        return (
            CandidateState::Quarantined,
            "cargo test/run gate did not pass".to_string(),
        );
    }
    if promotion_blockers
        .iter()
        .any(|blocker| blocker == "forbidden_target")
    {
        return (
            CandidateState::Blocked,
            "forbidden target blocker".to_string(),
        );
    }
    if replay_status == "ok" && summary.useful_change && promotion_blockers.is_empty() {
        return (
            CandidateState::Ready,
            "all required gates passed".to_string(),
        );
    }
    if !promotion_blockers.is_empty() {
        return (CandidateState::Blocked, promotion_blockers.join(", "));
    }
    (
        CandidateState::Unknown,
        "candidate state could not be determined".to_string(),
    )
}

fn summarize_queue(items: &[PromotionQueueItem]) -> CandidateQueueSummary {
    let mut summary = CandidateQueueSummary {
        candidate_count: items.len(),
        ..CandidateQueueSummary::default()
    };
    for item in items {
        match item.candidate_state {
            CandidateState::Ready => summary.ready_candidates += 1,
            CandidateState::Blocked => summary.blocked_candidates += 1,
            CandidateState::Quarantined => summary.quarantined_candidates += 1,
            CandidateState::Stale => summary.blocked_candidates += 1,
            CandidateState::Legacy => summary.legacy_candidates += 1,
            CandidateState::Duplicate => summary.duplicate_candidates += 1,
            CandidateState::Unreplayable => summary.unreplayable_candidates += 1,
            CandidateState::AlreadyPromoted => summary.already_promoted_candidates += 1,
            CandidateState::Unknown => summary.unknown_candidates += 1,
        }
    }
    summary
}

fn classify_lifecycle(
    summary: &memory::CandidateSummary,
    mutation_class: &str,
    replay_status: &str,
    promotion_blockers: &[String],
    promotion_allowed: bool,
    report_exists: bool,
) -> (String, String, String) {
    if !report_exists {
        return (
            "stale_report".to_string(),
            "stale_report".to_string(),
            "Отчёт кандидата отсутствует и не был восстановлен.".to_string(),
        );
    }
    match mutation_class {
        "cosmetic" => {
            return (
                "cosmetic_rejected".to_string(),
                "blocked".to_string(),
                "Косметическая мутация исключена из promotion queue.".to_string(),
            );
        }
        "unsafe" => {
            return (
                "unsafe_rejected".to_string(),
                "blocked".to_string(),
                "Опасная мутация исключена из promotion queue.".to_string(),
            );
        }
        "legacy" => {
            return (
                "unknown".to_string(),
                "blocked".to_string(),
                "Legacy-мутация не допускается к promotion.".to_string(),
            );
        }
        _ => {}
    }
    if !summary.useful_change {
        return (
            "blocked".to_string(),
            "blocked".to_string(),
            "Кандидат не отмечен как useful_change.".to_string(),
        );
    }
    if promotion_blockers
        .iter()
        .any(|blocker| blocker == "already_promoted")
    {
        return (
            "already_promoted".to_string(),
            "already_promoted".to_string(),
            "Кандидат уже был promoted ранее.".to_string(),
        );
    }
    if replay_status == "not_run" {
        return (
            "needs_replay".to_string(),
            "needs_replay".to_string(),
            "Для кандидата ещё не выполнен replay.".to_string(),
        );
    }
    if replay_status != "ok" {
        return (
            "replay_failed".to_string(),
            "blocked".to_string(),
            "Replay не подтверждён, promotion запрещён.".to_string(),
        );
    }
    if promotion_allowed {
        return (
            "ready".to_string(),
            "ready".to_string(),
            "Кандидат готов к ручному promotion-review.".to_string(),
        );
    }
    if !promotion_blockers.is_empty() {
        return (
            "blocked".to_string(),
            "blocked".to_string(),
            format!("Кандидат заблокирован: {}.", promotion_blockers.join(", ")),
        );
    }
    (
        "unknown".to_string(),
        "unknown".to_string(),
        "Состояние кандидата не удалось определить.".to_string(),
    )
}

fn write_promotion_queue(memory_root: &str, queue: &PromotionQueue) -> Result<(), String> {
    let json_path = Path::new(memory_root).join("promotion_queue.json");
    memory::write_json(&json_path, queue)?;
    let markdown_path = Path::new(memory_root).join("promotion_queue.ru.md");
    fs::write(&markdown_path, render_promotion_queue_markdown(queue))
        .map_err(|error| format!("failed to write promotion queue markdown: {error}"))
}

fn render_promotion_queue_markdown(queue: &PromotionQueue) -> String {
    let ready = queue.summary.ready_candidates;
    let blocked = queue.items.len().saturating_sub(ready);
    let lines = if queue.items.is_empty() {
        "(none)".to_string()
    } else {
        queue
            .items
            .iter()
            .map(|item| {
                format!(
                    "- {} kind={} class={} replay={} lifecycle={} state={:?} allowed={} target={} reason={}",
                    item.run_id,
                    item.mutation_kind,
                    item.mutation_class,
                    item.replay_status,
                    item.lifecycle_state,
                    item.candidate_state,
                    item.promotion_allowed,
                    item.target_file,
                    item.reason_ru
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "# Promotion Queue EVA\n\nitems={}\nready={}\nblocked={}\nquarantined={}\nlegacy={}\nduplicate={}\nunreplayable={}\nalready_promoted={}\nunknown={}\n\n{}\n",
        queue.items.len(),
        ready,
        blocked,
        queue.summary.quarantined_candidates,
        queue.summary.legacy_candidates,
        queue.summary.duplicate_candidates,
        queue.summary.unreplayable_candidates,
        queue.summary.already_promoted_candidates,
        queue.summary.unknown_candidates,
        lines
    )
}
