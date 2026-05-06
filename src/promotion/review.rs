use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::contracts::{MutationContract, MutationKind};
use crate::evolution::{autonomy_status, load_report_json};
use crate::promotion::gate::check_promotion_gate;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateReview {
    pub run_id: String,
    pub mutation_kind: String,
    pub target_file: String,
    pub score: f32,
    pub risk: f32,
    pub useful_change: bool,
    pub replay_status: String,
    pub report_path: String,
    pub promotion_allowed: bool,
    pub promotion_ready_reason: Option<String>,
    pub promotion_blockers: Vec<String>,
    pub russian_summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateReviewReport {
    pub run_id: String,
    pub promotion_allowed: bool,
    pub replay_status: String,
    pub promotion_ready_reason: Option<String>,
    pub promotion_blockers: Vec<String>,
    pub recommendation_ru: String,
    pub markdown_path: String,
}

pub fn review_candidate(
    project_root: &str,
    memory_root: &str,
    run_id: &str,
) -> Result<CandidateReview, String> {
    let summary = crate::evolution::memory::load_candidate_summary(memory_root, run_id).ok();
    let mutation = crate::evolution::memory::load_candidate(memory_root, run_id).ok();
    let replay_status = replay_status(memory_root, run_id)?;
    let report_path = Path::new(memory_root)
        .join("reports")
        .join(format!("{run_id}.ru.md"));
    let report_json_path = Path::new(memory_root)
        .join("reports")
        .join(format!("{run_id}.report.json"));

    let mutation_kind = summary
        .as_ref()
        .map(|value| value.mutation_kind.clone())
        .or_else(|| {
            mutation
                .as_ref()
                .map(|value| crate::evolution::memory::mutation_kind_label(value.kind))
        })
        .unwrap_or_else(|| "unknown".to_string());
    let target_file = summary
        .as_ref()
        .map(|value| value.target_file.clone())
        .or_else(|| mutation.as_ref().map(|value| value.target_file.clone()))
        .unwrap_or_else(|| "(missing)".to_string());
    let score = summary.as_ref().map(|value| value.score).unwrap_or(0.0);
    let risk = summary.as_ref().map(|value| value.risk).unwrap_or(0.0);
    let useful_change = summary
        .as_ref()
        .map(|value| value.useful_change)
        .unwrap_or(false);

    let blockers = collect_blockers(
        project_root,
        memory_root,
        run_id,
        summary.as_ref(),
        mutation.as_ref(),
        &replay_status,
        report_path.exists(),
        report_json_path.exists(),
    )?;
    let promotion_allowed = blockers.is_empty();
    let promotion_ready_reason = blockers.first().cloned();

    let review = CandidateReview {
        run_id: run_id.to_string(),
        mutation_kind: mutation_kind.clone(),
        target_file: target_file.clone(),
        score,
        risk,
        useful_change,
        replay_status: replay_status.clone(),
        report_path: report_path.display().to_string(),
        promotion_allowed,
        promotion_ready_reason,
        promotion_blockers: blockers,
        russian_summary: russian_summary(
            run_id,
            score,
            risk,
            useful_change,
            &replay_status,
            promotion_allowed,
            &target_file,
            &mutation_kind,
        ),
    };
    write_review_report(memory_root, &review)?;
    Ok(review)
}

pub fn candidate_diff(memory_root: &str, run_id: &str) -> Result<String, String> {
    let mutation = crate::evolution::memory::load_candidate(memory_root, run_id)?;
    Ok(render_diff(&mutation))
}

pub fn review_report_markdown(memory_root: &str, run_id: &str) -> Result<String, String> {
    let path = Path::new(memory_root)
        .join("reviews")
        .join(format!("{run_id}.ru.md"));
    fs::read_to_string(path).map_err(|error| format!("failed to read review report: {error}"))
}

fn write_review_report(memory_root: &str, review: &CandidateReview) -> Result<(), String> {
    let dir = Path::new(memory_root).join("reviews");
    fs::create_dir_all(&dir).map_err(|error| format!("failed to create reviews dir: {error}"))?;
    let markdown_path = dir.join(format!("{}.ru.md", review.run_id));
    let markdown = render_review_markdown(review);
    fs::write(&markdown_path, markdown)
        .map_err(|error| format!("failed to write review markdown: {error}"))?;
    let review_report = CandidateReviewReport {
        run_id: review.run_id.clone(),
        promotion_allowed: review.promotion_allowed,
        replay_status: review.replay_status.clone(),
        promotion_ready_reason: review.promotion_ready_reason.clone(),
        promotion_blockers: review.promotion_blockers.clone(),
        recommendation_ru: review.russian_summary.clone(),
        markdown_path: markdown_path.display().to_string(),
    };
    crate::evolution::memory::write_json(
        dir.join(format!("{}.review.json", review.run_id)),
        &review_report,
    )
}

fn collect_blockers(
    project_root: &str,
    memory_root: &str,
    _run_id: &str,
    summary: Option<&crate::evolution::CandidateSummary>,
    mutation: Option<&MutationContract>,
    replay_status: &str,
    report_markdown_exists: bool,
    report_json_exists: bool,
) -> Result<Vec<String>, String> {
    let mut blockers = Vec::new();
    if summary.is_none() {
        blockers.push("candidate_missing".to_string());
    }
    if mutation.is_none() {
        blockers.push("mutation_missing".to_string());
    }
    if !report_markdown_exists || !report_json_exists {
        blockers.push("report_missing".to_string());
    }

    if let Some(summary) = summary {
        if !summary.useful_change {
            blockers.push("useful_change_false".to_string());
        }
        if summary.score < crate::evolution::memory::PROMOTION_THRESHOLD {
            blockers.push("score_below_threshold".to_string());
        }
        if promoted_digest_exists(memory_root, &summary.mutation_digest)? {
            blockers.push("already_promoted".to_string());
        }
    }

    if replay_status != "ok" {
        blockers.push("replay_not_ok".to_string());
    }

    if let (Some(summary), Some(mutation)) = (summary, mutation) {
        if matches!(mutation.kind, MutationKind::AppendComment) {
            blockers.push("appendcomment_cosmetic".to_string());
        }

        let gate = check_promotion_gate(mutation, summary.score);
        if !gate.allowed {
            if gate.reason.contains("forbidden") || gate.reason.contains("core/main/lib") {
                blockers.push("forbidden_target".to_string());
            } else if gate.reason.contains("score") {
                blockers.push("score_below_threshold".to_string());
            } else {
                blockers.push("promotion_gate_blocked".to_string());
            }
        }

        let target_path = Path::new(project_root).join(&mutation.target_file);
        if payload_already_present(&target_path, mutation)? {
            blockers.push("target_already_contains_payload".to_string());
        }
        if duplicate_test_function_name(&target_path, mutation)? {
            blockers.push("duplicate_test_function_name".to_string());
        }
    }

    let autonomy = autonomy_status(project_root, memory_root)?;
    if !autonomy.blockers.is_empty() {
        blockers.push("autonomy_blocked".to_string());
    }

    blockers.sort();
    blockers.dedup();
    Ok(blockers)
}

fn replay_status(memory_root: &str, run_id: &str) -> Result<String, String> {
    if let Ok(report) = load_report_json(memory_root, run_id) {
        return Ok(report.replay_status);
    }
    let path = Path::new(memory_root)
        .join("replays")
        .join(format!("{run_id}.json"));
    if !path.exists() {
        return Ok("not_run".to_string());
    }
    let contents =
        fs::read_to_string(path).map_err(|error| format!("failed to read replay: {error}"))?;
    let replay: crate::evolution::ReplayResult = serde_json::from_str(&contents)
        .map_err(|error| format!("failed to parse replay: {error}"))?;
    let passed = replay.matches_stored_summary
        && replay.cargo_check_ok
        && replay.cargo_test_ok
        && replay.cargo_run_ok
        && replay.replay_status != crate::contracts::EvolutionStatus::Failed;
    Ok(if passed { "ok" } else { "failed" }.to_string())
}

fn render_diff(mutation: &MutationContract) -> String {
    match mutation.kind {
        MutationKind::AddUnitTest
        | MutationKind::AddReplayAssertion
        | MutationKind::AppendComment => {
            format!(
                "target: {}\nkind: {:?}\nappend:\n{}\n",
                mutation.target_file,
                mutation.kind,
                mutation.append.as_deref().unwrap_or("(none)")
            )
        }
        MutationKind::ReplaceText
        | MutationKind::ParameterTune
        | MutationKind::AddLearningSummaryField
        | MutationKind::AddMetricUpdate => format!(
            "target: {}\nkind: {:?}\nsearch:\n{}\n\nreplace:\n{}\n",
            mutation.target_file,
            mutation.kind,
            mutation.search.as_deref().unwrap_or("(none)"),
            mutation.replace.as_deref().unwrap_or("(none)")
        ),
        MutationKind::AddTestSkeleton | MutationKind::AddMetricField => format!(
            "target: {}\nkind: {:?}\npayload:\n{}\n",
            mutation.target_file,
            mutation.kind,
            mutation.append.as_deref().unwrap_or("(none)")
        ),
    }
}

fn russian_summary(
    run_id: &str,
    score: f32,
    risk: f32,
    useful_change: bool,
    replay_status: &str,
    promotion_allowed: bool,
    target_file: &str,
    mutation_kind: &str,
) -> String {
    format!(
        "Кандидат {} меняет {} через {}. Score {:.1}, risk {:.2}, useful={}, replay={}, promotion_ready={}.",
        run_id, target_file, mutation_kind, score, risk, useful_change, replay_status, promotion_allowed
    )
}

fn render_review_markdown(review: &CandidateReview) -> String {
    let blockers = if review.promotion_blockers.is_empty() {
        "Нет blocker reason.".to_string()
    } else {
        review
            .promotion_blockers
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "# Review EVA\n\n## Кандидат\nrun_id: {}\nkind: {}\nfile: {}\n\n## Что изменено\n{}\n\n## Почему полезно\nuseful_change={}\nscore={:.1}\n\n## Проверки\nrisk={:.2}\nreport={}\n\n## Replay\nstatus={}\n\n## Риск\nОценка риска {:.2}\n\n## Готовность к promotion\npromotion_ready={}\nreason={}\n{}\n\n## Рекомендация EVA\n{}\n",
        review.run_id,
        review.mutation_kind,
        review.target_file,
        review.target_file,
        review.useful_change,
        review.score,
        review.risk,
        review.report_path,
        review.replay_status,
        review.risk,
        review.promotion_allowed,
        review
            .promotion_ready_reason
            .clone()
            .unwrap_or_else(|| "ready".to_string()),
        blockers,
        review.russian_summary
    )
}

fn promoted_digest_exists(memory_root: &str, mutation_digest: &str) -> Result<bool, String> {
    if mutation_digest.is_empty() {
        return Ok(false);
    }
    let path = Path::new(memory_root).join("evolution.jsonl");
    if !path.exists() {
        return Ok(false);
    }
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read evolution log: {error}"))?;
    Ok(contents
        .lines()
        .filter_map(|line| serde_json::from_str::<crate::contracts::EvolutionLogEntry>(line).ok())
        .any(|entry| entry.retained_in_core && entry.mutation_digest == mutation_digest))
}

fn payload_already_present(
    target_path: &Path,
    mutation: &MutationContract,
) -> Result<bool, String> {
    if !target_path.exists() {
        return Ok(false);
    }
    let content = fs::read_to_string(target_path)
        .map_err(|error| format!("failed to read promotion target: {error}"))?;
    let payload = mutation
        .append
        .as_deref()
        .or(mutation.replace.as_deref())
        .unwrap_or("");
    Ok(!payload.is_empty() && content.contains(payload))
}

fn duplicate_test_function_name(
    target_path: &Path,
    mutation: &MutationContract,
) -> Result<bool, String> {
    if !target_path.exists() {
        return Ok(false);
    }
    let Some(payload) = mutation.append.as_deref() else {
        return Ok(false);
    };
    let Some(function_name) = payload.lines().find_map(extract_test_fn_name) else {
        return Ok(false);
    };
    let content = fs::read_to_string(target_path)
        .map_err(|error| format!("failed to read test target: {error}"))?;
    let needle = format!("fn {function_name}");
    Ok(content.contains(&needle))
}

fn extract_test_fn_name(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let remainder = trimmed.strip_prefix("fn ")?;
    let name = remainder
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect::<String>();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}
