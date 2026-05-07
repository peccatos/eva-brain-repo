use std::fs;
use std::path::Path;

use crate::contracts::{
    sha256_digest, OperatorApprovalRecord, PromotionQueueItem, ReleasePreflightReport,
};
use crate::evolution::{
    candidate_lifecycle, classify_mutation_kind_label, latest_record_for_run, memory,
    mutation_class_label, refresh_promotion_queue,
};
use crate::promotion::review::candidate_diff;

#[derive(Debug, Clone)]
pub(crate) struct ReleaseCandidateContext {
    pub run_id: String,
    pub mutation_kind: String,
    pub mutation_class: String,
    pub target_file: String,
    pub replay_status: String,
    pub approval_required: bool,
    pub approved: bool,
    pub promotion_queue_state: String,
    pub risk: f32,
    pub score: f32,
    pub useful_change: bool,
    pub candidate_report_path: Option<String>,
    pub generated_at: u64,
    pub candidate_diff_summary: Option<String>,
    pub queue_item: Option<PromotionQueueItem>,
    pub approval_record: Option<OperatorApprovalRecord>,
    pub summary_exists: bool,
}

pub fn build_release_preflight(
    project_root: &str,
    memory_root: &str,
    run_id: &str,
) -> Result<ReleasePreflightReport, String> {
    let context = load_release_candidate_context(project_root, memory_root, run_id)?;
    let report = build_release_preflight_from_context(&context);
    write_release_preflight(memory_root, &report)?;
    Ok(report)
}

pub fn print_release_preflight(
    project_root: &str,
    memory_root: &str,
    run_id: &str,
) -> Result<String, String> {
    let report = build_release_preflight(project_root, memory_root, run_id)?;
    serde_json::to_string_pretty(&report)
        .map_err(|error| format!("failed to serialize release preflight: {error}"))
}

pub fn print_release_preflight_json(
    project_root: &str,
    memory_root: &str,
    run_id: &str,
) -> Result<String, String> {
    print_release_preflight(project_root, memory_root, run_id)
}

fn build_release_preflight_from_context(
    context: &ReleaseCandidateContext,
) -> ReleasePreflightReport {
    let mut blockers = Vec::new();
    if !context.summary_exists {
        blockers.push("candidate_not_found".to_string());
    }
    if !context.useful_change {
        blockers.push("useful_change_false".to_string());
    }
    if context.mutation_kind == "appendcomment" {
        blockers.push("appendcomment_cosmetic".to_string());
    }
    if context.mutation_class == "cosmetic" {
        blockers.push("cosmetic_mutation".to_string());
    }
    if context.mutation_class == "unsafe" {
        blockers.push("unsafe_mutation".to_string());
    }
    if context.mutation_class == "legacy" {
        blockers.push("legacy_mutation".to_string());
    }
    if context.replay_status != "ok" {
        blockers.push("replay_not_ok".to_string());
    }
    if context.approval_required && !context.approved {
        blockers.push("approval_required".to_string());
        blockers.push("not_approved".to_string());
    }
    if let Some(item) = &context.queue_item {
        if item.lifecycle_state != "ready" {
            blockers.push("promotion_queue_blocked".to_string());
            blockers.push(item.lifecycle_state.clone());
        }
        if item
            .promotion_blockers
            .iter()
            .any(|blocker| blocker == "forbidden_target")
        {
            blockers.push("forbidden_target".to_string());
        }
    } else {
        blockers.push("promotion_queue_blocked".to_string());
    }
    if context.target_file.starts_with("src/core/")
        || context.target_file == "src/main.rs"
        || context.target_file == "src/lib.rs"
        || context.target_file == "Cargo.toml"
    {
        blockers.push("forbidden_target".to_string());
    }
    if context.candidate_diff_summary.is_none() {
        blockers.push("missing_diff".to_string());
    }

    blockers.sort();
    blockers.dedup();
    let allowed = blockers.is_empty();
    ReleasePreflightReport {
        run_id: context.run_id.clone(),
        allowed,
        reason_ru: if allowed {
            "Release preflight разрешил metadata-only bundle.".to_string()
        } else {
            format!(
                "Release preflight заблокировал bundle: {}.",
                blockers.join(", ")
            )
        },
        blockers,
        mutation_kind: context.mutation_kind.clone(),
        mutation_class: context.mutation_class.clone(),
        target_file: context.target_file.clone(),
        replay_status: context.replay_status.clone(),
        approval_required: context.approval_required,
        approved: context.approved,
        promotion_queue_state: context.promotion_queue_state.clone(),
        risk: context.risk,
        score: context.score,
        generated_at: context.generated_at,
    }
}

pub fn release_id_from_preflight(report: &ReleasePreflightReport) -> String {
    let seed = format!(
        "{}:{}:{}:{}:{:.3}:{:.3}:{}:{}:{}",
        report.run_id,
        report.target_file,
        report.mutation_kind,
        report.mutation_class,
        report.score,
        report.risk,
        report.replay_status,
        report.approved,
        report.promotion_queue_state
    );
    format!(
        "release-{}-{}",
        &sha256_digest(&seed)[..8],
        normalize_release_id_fragment(&report.run_id)
    )
}

pub(crate) fn load_release_candidate_context(
    project_root: &str,
    memory_root: &str,
    run_id: &str,
) -> Result<ReleaseCandidateContext, String> {
    let _ = refresh_promotion_queue(project_root, memory_root);
    let summary = memory::load_candidate_summary(memory_root, run_id).ok();
    let mutation = memory::load_candidate(memory_root, run_id).ok();
    let report = crate::evolution::load_report_json(memory_root, run_id).ok();
    let queue_item = candidate_lifecycle(project_root, memory_root, run_id).ok();
    let approval_record = latest_record_for_run(memory_root, run_id).ok().flatten();

    let mutation_kind = summary
        .as_ref()
        .map(|value| value.mutation_kind.clone())
        .or_else(|| {
            mutation
                .as_ref()
                .map(|value| crate::evolution::memory::mutation_kind_label(value.kind))
        })
        .unwrap_or_else(|| "unknown".to_string());
    let useful_change = summary
        .as_ref()
        .map(|value| value.useful_change)
        .unwrap_or_else(|| {
            report
                .as_ref()
                .map(|value| value.mutation_class == "useful")
                .unwrap_or(false)
        });
    let mutation_class = report
        .as_ref()
        .map(|value| value.mutation_class.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            mutation_class_label(classify_mutation_kind_label(&mutation_kind, useful_change))
                .to_string()
        });
    let target_file = summary
        .as_ref()
        .map(|value| value.target_file.clone())
        .or_else(|| mutation.as_ref().map(|value| value.target_file.clone()))
        .or_else(|| report.as_ref().map(|value| value.target_file.clone()))
        .unwrap_or_default();
    let replay_status = report
        .as_ref()
        .map(|value| value.replay_status.clone())
        .unwrap_or_else(|| {
            queue_item
                .as_ref()
                .map(|item| item.replay_status.clone())
                .unwrap_or_else(|| "unknown".to_string())
        });
    let approved = approval_record
        .as_ref()
        .is_some_and(|record| record.decision == "approve");
    let approval_required = true;
    let candidate_report_path = candidate_report_path(memory_root, run_id);
    let candidate_diff_summary = mutation
        .as_ref()
        .and_then(|_| candidate_diff(memory_root, run_id).ok());
    let score = summary.as_ref().map(|value| value.score).unwrap_or(0.0);
    let risk = summary
        .as_ref()
        .map(|value| value.risk)
        .or_else(|| mutation.as_ref().map(|value| value.risk))
        .unwrap_or(0.0);
    let generated_at = [
        summary
            .as_ref()
            .map(|value| value.timestamp_unix)
            .unwrap_or(0),
        approval_record
            .as_ref()
            .map(|value| value.created_at)
            .unwrap_or(0),
        report
            .as_ref()
            .and_then(|value| value.replay_checked_at)
            .unwrap_or(0),
    ]
    .into_iter()
    .max()
    .unwrap_or(0);

    Ok(ReleaseCandidateContext {
        run_id: run_id.to_string(),
        mutation_kind,
        mutation_class,
        target_file,
        replay_status,
        approval_required,
        approved,
        promotion_queue_state: queue_item
            .as_ref()
            .map(|item| item.lifecycle_state.clone())
            .unwrap_or_else(|| "unknown".to_string()),
        risk,
        score,
        useful_change,
        candidate_report_path,
        generated_at,
        candidate_diff_summary,
        queue_item,
        approval_record,
        summary_exists: summary.is_some(),
    })
}

pub(crate) fn candidate_report_path(memory_root: &str, run_id: &str) -> Option<String> {
    let markdown = Path::new(memory_root)
        .join("reports")
        .join(format!("{run_id}.ru.md"));
    if markdown.exists() {
        return Some(markdown.display().to_string());
    }
    let json = Path::new(memory_root)
        .join("reports")
        .join(format!("{run_id}.report.json"));
    if json.exists() {
        return Some(json.display().to_string());
    }
    None
}

fn normalize_release_id_fragment(run_id: &str) -> String {
    let normalized = run_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    normalized.trim_matches('-').to_string()
}

fn write_release_preflight(
    memory_root: &str,
    report: &ReleasePreflightReport,
) -> Result<(), String> {
    let release_id = release_id_from_preflight(report);
    let dir = Path::new(memory_root).join("releases").join("preflight");
    fs::create_dir_all(&dir)
        .map_err(|error| format!("failed to create release preflight dir: {error}"))?;
    memory::write_json(dir.join(format!("{release_id}.preflight.json")), report)
}
