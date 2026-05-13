use std::fs;
use std::path::{Path, PathBuf};

use crate::contracts::{
    CandidateState, EvolutionLogEntry, EvolutionReport, PromotionQueue, PromotionQueueItem,
    RuntimeCandidateManifest, RuntimeServiceMetadata, TuiCandidateRow, TuiDashboardState,
    TuiMetricsState, TuiReleaseState, TuiRunRow, TuiState,
};
use crate::evolution::{
    autonomy_status, count_sandbox_leaks, load_latest_runtime_validation, load_metrics_snapshot,
    load_promotion_queue, load_report_json, ReplayResult,
};

pub fn load_tui_state(project_root: &str, _memory_root: &str) -> TuiState {
    load_tui_state_from_project_root(Path::new(project_root))
}

pub fn load_tui_state_from_project_root(root: &Path) -> TuiState {
    let memory_root = root.join("memory");
    let mut parse_messages = Vec::new();

    let metrics = load_metrics_from_file(&memory_root, &mut parse_messages);
    let queue = load_queue_from_file(&memory_root, &mut parse_messages);
    let validation = match load_latest_runtime_validation(memory_root.to_str().unwrap_or("memory"))
    {
        Ok(value) => value,
        Err(error) => {
            parse_messages.push(parse_error_message(
                &memory_root.join("runtime_validation"),
                &error,
            ));
            None
        }
    };
    let runtime_candidate = load_latest_json::<RuntimeCandidateManifest>(
        &memory_root.join("runtime_candidates"),
        "runtime candidate",
        &mut parse_messages,
    );
    let _runtime_service = load_exact_json::<RuntimeServiceMetadata>(
        &memory_root.join("runtime_service").join("eva-runtime.json"),
        "runtime service",
        &mut parse_messages,
    );
    let autonomy = autonomy_status(
        root.to_str().unwrap_or("."),
        memory_root.to_str().unwrap_or("memory"),
    )
    .ok();
    let runs = load_run_rows(&memory_root, &mut parse_messages);
    let candidates = build_candidate_rows(&queue);
    let last_replay_status = resolve_last_replay_status(
        &memory_root,
        metrics.last_run_id.as_deref(),
        &queue.items,
        &mut parse_messages,
    );

    let validation_status = validation
        .as_ref()
        .map(|value| format_unknown(&value.status))
        .unwrap_or_else(|| "missing".to_string());
    let release_status = runtime_candidate
        .as_ref()
        .map(|value| format_unknown(&value.rc_status))
        .or_else(|| {
            validation
                .as_ref()
                .map(|value| format_unknown(&value.status))
        })
        .unwrap_or_else(|| "missing".to_string());

    let blocked_candidates = queue
        .summary
        .blocked_candidates
        .saturating_add(queue.summary.already_promoted_candidates)
        .saturating_add(queue.summary.legacy_candidates)
        .saturating_add(queue.summary.unknown_candidates);

    let dashboard = TuiDashboardState {
        runtime_status: validation_status.clone(),
        runtime_validation_status: validation_status.clone(),
        autonomy_level: autonomy
            .as_ref()
            .map(|value| value.current_level)
            .unwrap_or(0),
        allowed_next_autonomy_level: autonomy
            .as_ref()
            .map(|value| value.allowed_next_level)
            .unwrap_or(0),
        campaign_mode_allowed: autonomy
            .as_ref()
            .map(|value| value.campaign_mode_allowed)
            .unwrap_or(false),
        latest_run_id: metrics.last_run_id.clone(),
        last_replay_status,
        candidate_count: candidate_count(&metrics, &queue),
        ready_candidates: queue.summary.ready_candidates,
        blocked_candidates,
        quarantined_candidates: queue.summary.quarantined_candidates,
        duplicate_candidates: queue.summary.duplicate_candidates,
        unreplayable_candidates: queue.summary.unreplayable_candidates,
        release_status: release_status.clone(),
        warnings: validation
            .as_ref()
            .map(|value| value.warnings.clone())
            .unwrap_or_default(),
        missing_green_conditions: validation
            .as_ref()
            .map(|value| value.missing_green_conditions.clone())
            .unwrap_or_default(),
        blockers: validation
            .as_ref()
            .map(|value| value.blockers.clone())
            .unwrap_or_default(),
        sandbox_leak_count: count_sandbox_leaks(root.to_str().unwrap_or(".")).unwrap_or_default()
            as usize,
    };

    let release = TuiReleaseState {
        approved_release_candidate_exists: validation
            .as_ref()
            .and_then(|value| value.approved_release_candidate.as_ref())
            .is_some(),
        release_bundle_exists: validation
            .as_ref()
            .and_then(|value| value.release_bundle.as_ref())
            .is_some(),
        latest_release_candidate: validation
            .as_ref()
            .and_then(|value| value.approved_release_candidate.clone()),
        operator_approval_state: validation
            .as_ref()
            .map(|value| {
                if value.approved_release_candidate.is_some() {
                    "approved".to_string()
                } else {
                    "required".to_string()
                }
            })
            .unwrap_or_else(|| "missing".to_string()),
        preflight_gate_status: runtime_candidate
            .as_ref()
            .map(|value| extract_keyed_status(&value.preflight_gate_v3_state))
            .unwrap_or_else(|| "missing".to_string()),
        release_health: runtime_candidate
            .as_ref()
            .map(|value| extract_release_health(&value.operations_state))
            .unwrap_or_else(|| "missing".to_string()),
        green_gate_readiness: if validation_status == "green" {
            "green".to_string()
        } else {
            validation_status.clone()
        },
        warnings: dashboard.warnings.clone(),
        missing_green_conditions: dashboard.missing_green_conditions.clone(),
        blockers: dashboard.blockers.clone(),
    };

    let logs = build_logs(
        &parse_messages,
        validation
            .as_ref()
            .map(|value| value.warnings.as_slice())
            .unwrap_or(&[]),
        validation
            .as_ref()
            .map(|value| value.blockers.as_slice())
            .unwrap_or(&[]),
        validation
            .as_ref()
            .map(|value| value.checks.as_slice())
            .unwrap_or(&[]),
    );

    TuiState {
        dashboard,
        runs,
        candidates,
        metrics: TuiMetricsState {
            total_runs: metrics.total_runs,
            passed_runs: metrics.passed_runs,
            failed_runs: metrics.failed_runs,
            safety_rejected_runs: metrics.safety_rejected_runs,
            duplicate_rejected_runs: metrics.duplicate_rejected_runs,
            replay_passed: metrics.replay_passed,
            replay_failed: metrics.replay_failed,
            candidate_count: candidate_count(&metrics, &queue),
            promoted_count: metrics.promoted_count,
            average_score: metrics.average_score,
            pass_ratio: metrics.pass_ratio,
            replay_pass_ratio: replay_pass_ratio(&metrics),
        },
        release,
        logs,
        parse_messages,
    }
}

pub fn format_unknown(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "unknown".to_string()
    } else {
        trimmed.to_string()
    }
}

fn load_metrics_from_file(
    memory_root: &Path,
    parse_messages: &mut Vec<String>,
) -> crate::evolution::EvolutionMetrics {
    match load_metrics_snapshot(memory_root.to_str().unwrap_or("memory")) {
        Ok(metrics) => metrics,
        Err(error) => {
            parse_messages.push(parse_error_message(
                &memory_root.join("metrics.json"),
                &error,
            ));
            crate::evolution::EvolutionMetrics::default()
        }
    }
}

fn load_queue_from_file(memory_root: &Path, parse_messages: &mut Vec<String>) -> PromotionQueue {
    match load_promotion_queue(memory_root.to_str().unwrap_or("memory")) {
        Ok(queue) => queue,
        Err(error) => {
            parse_messages.push(parse_error_message(
                &memory_root.join("promotion_queue.json"),
                &error,
            ));
            PromotionQueue::default()
        }
    }
}

fn load_run_rows(memory_root: &Path, parse_messages: &mut Vec<String>) -> Vec<TuiRunRow> {
    let path = memory_root.join("evolution.jsonl");
    if !path.exists() {
        return Vec::new();
    }
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) => {
            parse_messages.push(parse_error_message(&path, &error.to_string()));
            return Vec::new();
        }
    };
    let mut runs = Vec::new();
    for (index, line) in contents.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let entry = match serde_json::from_str::<EvolutionLogEntry>(line) {
            Ok(entry) => entry,
            Err(error) => {
                parse_messages.push(format!("parse_error:{}:{}", path.display(), error));
                if index == 0 {
                    continue;
                }
                continue;
            }
        };
        let replay_status =
            load_report_json(memory_root.to_str().unwrap_or("memory"), &entry.run_id)
                .map(|report| normalize_report_replay_status(&report))
                .unwrap_or_else(|_| "missing".to_string());
        runs.push(TuiRunRow {
            run_id: entry.run_id.clone(),
            status: format!("{:?}", entry.status).to_ascii_lowercase(),
            replay_status,
            cargo_test_ok: Some(entry.cargo_test_ok),
            cargo_run_ok: Some(entry.cargo_run_ok),
            duplicate_rejected: entry.duplicate_rejected,
            candidate: matches!(entry.status, crate::contracts::EvolutionStatus::Candidate),
            promoted: entry.retained_in_core
                || matches!(entry.status, crate::contracts::EvolutionStatus::Promoted),
            reason: entry
                .non_candidate_reason
                .clone()
                .unwrap_or_else(|| "none".to_string()),
        });
    }
    runs.sort_by(|left, right| right.run_id.cmp(&left.run_id));
    runs
}

fn build_candidate_rows(queue: &PromotionQueue) -> Vec<TuiCandidateRow> {
    let mut rows = queue
        .items
        .iter()
        .map(|item| TuiCandidateRow {
            run_id: item.run_id.clone(),
            state: candidate_state_label(&item.candidate_state),
            mutation_kind: item.mutation_kind.clone(),
            mutation_class: item.mutation_class.clone(),
            target_file: item.target_file.clone(),
            score: item.score,
            risk: item.risk,
            promotion_eligibility: item.promotion_state.clone(),
            promotion_allowed: item.promotion_allowed,
            replay_status: normalize_queue_replay_status(&item.replay_status),
            block_reason: if item.candidate_state_reason.trim().is_empty() {
                item.reason_ru.clone()
            } else {
                item.candidate_state_reason.clone()
            },
            cargo_test_ok: item.cargo_test_ok,
            cargo_run_ok: item.cargo_run_ok,
            duplicate_rejected: item.duplicate_rejected,
            promoted: item.promoted,
            updated_at: item.updated_at,
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left.run_id.cmp(&right.run_id));
    rows
}

fn resolve_last_replay_status(
    memory_root: &Path,
    latest_run_id: Option<&str>,
    items: &[PromotionQueueItem],
    parse_messages: &mut Vec<String>,
) -> String {
    if let Some(run_id) = latest_run_id {
        let replay_path = memory_root.join("replays").join(format!("{run_id}.json"));
        if replay_path.exists() {
            if let Some(status) = load_replay_status_from_path(&replay_path, parse_messages) {
                return status;
            }
        }
        if let Some(item) = items.iter().find(|item| item.run_id == run_id) {
            return normalize_queue_replay_status(&item.replay_status);
        }
    }

    if let Some(latest) = latest_replay_path(&memory_root.join("replays")) {
        if let Some(status) = load_replay_status_from_path(&latest, parse_messages) {
            return status;
        }
    }

    "missing".to_string()
}

fn load_replay_status_from_path(path: &Path, parse_messages: &mut Vec<String>) -> Option<String> {
    match load_exact_json::<ReplayResult>(path, "replay", parse_messages) {
        Some(replay) => Some(normalize_replay_result_status(&replay)),
        None if path.exists() => Some("parse_error".to_string()),
        None => None,
    }
}

fn latest_replay_path(dir: &Path) -> Option<PathBuf> {
    let mut entries = fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect::<Vec<_>>();
    entries.sort();
    entries.pop()
}

fn build_logs(
    parse_messages: &[String],
    warnings: &[String],
    blockers: &[String],
    checks: &[String],
) -> Vec<String> {
    let mut logs = Vec::new();
    logs.extend(parse_messages.iter().cloned());
    logs.extend(warnings.iter().map(|warning| format!("warning:{warning}")));
    logs.extend(blockers.iter().map(|blocker| format!("blocker:{blocker}")));
    logs.extend(
        checks
            .iter()
            .take(10)
            .map(|check| format!("validation:{check}")),
    );
    logs
}

fn candidate_count(metrics: &crate::evolution::EvolutionMetrics, queue: &PromotionQueue) -> u64 {
    if metrics.candidate_count > 0 {
        metrics.candidate_count
    } else {
        queue.summary.candidate_count as u64
    }
}

fn replay_pass_ratio(metrics: &crate::evolution::EvolutionMetrics) -> f32 {
    let total = metrics.replay_passed + metrics.replay_failed;
    if total == 0 {
        0.0
    } else {
        metrics.replay_passed as f32 / total as f32
    }
}

fn candidate_state_label(state: &CandidateState) -> String {
    match state {
        CandidateState::Ready => "ready",
        CandidateState::Blocked => "blocked",
        CandidateState::Quarantined => "quarantined",
        CandidateState::Stale => "stale",
        CandidateState::Legacy => "legacy",
        CandidateState::Duplicate => "duplicate",
        CandidateState::Unreplayable => "unreplayable",
        CandidateState::AlreadyPromoted => "already_promoted",
        CandidateState::Unknown => "unknown",
    }
    .to_string()
}

fn normalize_queue_replay_status(status: &str) -> String {
    match status {
        "ok" | "passed" | "candidate" | "promoted" => "passed".to_string(),
        "failed" => "failed".to_string(),
        "not_run" => "not_run".to_string(),
        value if value.trim().is_empty() => "unknown".to_string(),
        value => value.to_string(),
    }
}

fn normalize_replay_result_status(replay: &ReplayResult) -> String {
    match replay.replay_status {
        crate::contracts::EvolutionStatus::Failed => "failed".to_string(),
        crate::contracts::EvolutionStatus::Passed
        | crate::contracts::EvolutionStatus::Candidate
        | crate::contracts::EvolutionStatus::Promoted => "passed".to_string(),
    }
}

fn normalize_report_replay_status(report: &EvolutionReport) -> String {
    normalize_queue_replay_status(&report.replay_status)
}

fn extract_keyed_status(value: &str) -> String {
    value
        .split_whitespace()
        .find_map(|part| part.strip_prefix("status="))
        .map(str::to_string)
        .unwrap_or_else(|| format_unknown(value))
}

fn extract_release_health(value: &str) -> String {
    value
        .split_whitespace()
        .find_map(|part| part.strip_prefix("health="))
        .map(str::to_string)
        .unwrap_or_else(|| "missing".to_string())
}

fn parse_error_message(path: &Path, error: &str) -> String {
    format!("parse_error:{}:{error}", path.display())
}

fn load_exact_json<T: serde::de::DeserializeOwned>(
    path: &Path,
    label: &str,
    parse_messages: &mut Vec<String>,
) -> Option<T> {
    if !path.exists() {
        return None;
    }
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) => {
            parse_messages.push(parse_error_message(
                path,
                &format!("failed to read {label}: {error}"),
            ));
            return None;
        }
    };
    match serde_json::from_str::<T>(&contents) {
        Ok(value) => Some(value),
        Err(error) => {
            parse_messages.push(parse_error_message(
                path,
                &format!("failed to parse {label}: {error}"),
            ));
            None
        }
    }
}

fn load_latest_json<T: serde::de::DeserializeOwned>(
    dir: &Path,
    label: &str,
    parse_messages: &mut Vec<String>,
) -> Option<T> {
    if !dir.exists() {
        return None;
    }
    let mut paths = match fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .collect::<Vec<_>>(),
        Err(error) => {
            parse_messages.push(parse_error_message(
                dir,
                &format!("failed to read {label} dir: {error}"),
            ));
            return None;
        }
    };
    paths.sort();
    let path = paths.pop()?;
    load_exact_json(&path, label, parse_messages)
}
