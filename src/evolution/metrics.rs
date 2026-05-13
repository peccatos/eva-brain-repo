use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::contracts::{EvolutionLogEntry, EvolutionStatus, MutationContract};
use crate::evolution::memory::ReplayResult;
use crate::evolution::{dedup, memory, regression_memory, success_memory};

pub const DEFAULT_METRICS_PATH: &str = "memory/metrics.json";
pub const EVA_REPORTS_DIR: &str = "memory/reports";
pub const EVA_CANDIDATE_DIR: &str = "memory/candidates";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct EvolutionMetrics {
    #[serde(default)]
    pub total_runs: u64,
    #[serde(default)]
    pub passed_runs: u64,
    #[serde(default)]
    pub failed_runs: u64,
    #[serde(default)]
    pub real_execution_failed_runs: u64,
    #[serde(default)]
    pub cargo_gate_failed_runs: u64,
    #[serde(default)]
    pub replay_failed_runs: u64,
    #[serde(default)]
    pub safety_rejected_runs: u64,
    #[serde(default)]
    pub duplicate_rejected_runs: u64,
    #[serde(default)]
    pub cosmetic_rejected_runs: u64,
    #[serde(default)]
    pub already_promoted_runs: u64,
    #[serde(default)]
    pub policy_rejected_runs: u64,
    #[serde(default)]
    pub unknown_runs: u64,
    #[serde(default)]
    pub candidate_count: u64,
    #[serde(default)]
    pub replay_passed: u64,
    #[serde(default)]
    pub replay_failed: u64,
    #[serde(default)]
    pub promoted_count: u64,
    #[serde(default)]
    pub average_score: f32,
    #[serde(default)]
    pub pass_ratio: f32,
    #[serde(default)]
    pub effective_failure_ratio: f32,
    #[serde(default)]
    pub safety_rejection_ratio: f32,
    #[serde(default)]
    pub last_run_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvolutionRunOutcome {
    Passed,
    RealExecutionFailure,
    CargoGateFailure,
    ReplayFailure,
    DuplicateSafetyRejection,
    CosmeticRejection,
    AlreadyPromoted,
    PolicyRejection,
    BlockedByOperator,
    Unknown,
}

pub fn load_metrics(memory_root: &str) -> Result<EvolutionMetrics, String> {
    refresh_metrics(memory_root)
}

pub fn load_metrics_snapshot(memory_root: &str) -> Result<EvolutionMetrics, String> {
    let path = Path::new(memory_root).join("metrics.json");
    if !path.exists() {
        return Ok(EvolutionMetrics::default());
    }
    let contents =
        fs::read_to_string(&path).map_err(|error| format!("failed to read metrics: {error}"))?;
    serde_json::from_str(&contents).map_err(|error| format!("failed to parse metrics: {error}"))
}

pub fn update_metrics_after_log(
    memory_root: &str,
    entry: &EvolutionLogEntry,
) -> Result<EvolutionMetrics, String> {
    let mut metrics = load_metrics_snapshot(memory_root)?;
    let previous_total = metrics.total_runs;
    metrics.total_runs += 1;
    apply_outcome_counts(&mut metrics, classify_run_outcome(entry));
    if entry.status == EvolutionStatus::Candidate {
        metrics.candidate_count += 1;
    }
    if entry.status == EvolutionStatus::Promoted || entry.retained_in_core {
        metrics.promoted_count += 1;
    }
    metrics.average_score =
        ((metrics.average_score * previous_total as f32) + entry.score) / metrics.total_runs as f32;
    recompute_ratios(&mut metrics);
    metrics.last_run_id = Some(entry.run_id.clone());
    write_metrics(memory_root, &metrics)?;
    Ok(metrics)
}

pub fn update_metrics_after_replay(
    memory_root: &str,
    replay: &ReplayResult,
) -> Result<EvolutionMetrics, String> {
    let mut metrics = load_metrics_snapshot(memory_root)?;
    if replay_is_ok(replay) {
        metrics.replay_passed += 1;
    } else {
        metrics.replay_failed += 1;
        metrics.replay_failed_runs += 1;
        metrics.failed_runs += 1;
    }
    recompute_ratios(&mut metrics);
    write_metrics(memory_root, &metrics)?;
    Ok(metrics)
}

pub fn refresh_metrics(memory_root: &str) -> Result<EvolutionMetrics, String> {
    let logs = load_logs(memory_root)?;
    let summaries = memory::list_candidate_summaries(memory_root)?;
    let replays = load_replays(memory_root)?;

    let mut metrics = EvolutionMetrics {
        total_runs: logs.len() as u64,
        ..EvolutionMetrics::default()
    };
    for entry in &logs {
        apply_outcome_counts(&mut metrics, classify_run_outcome(entry));
    }
    let candidate_count = summaries.len() as u64;
    let replay_passed = replays.iter().filter(|replay| replay_is_ok(replay)).count() as u64;
    let replay_failed = replays
        .iter()
        .filter(|replay| !replay_is_ok(replay))
        .count() as u64;
    let promoted_count = logs.iter().filter(|entry| entry.retained_in_core).count() as u64;
    let average_score = if logs.is_empty() {
        0.0
    } else {
        logs.iter().map(|entry| entry.score).sum::<f32>() / logs.len() as f32
    };
    let last_run_id = logs.last().map(|entry| entry.run_id.clone());
    metrics.candidate_count = candidate_count;
    metrics.replay_passed = replay_passed;
    metrics.replay_failed = replay_failed;
    metrics.replay_failed_runs = replay_failed;
    metrics.failed_runs += replay_failed;
    metrics.promoted_count = promoted_count;
    metrics.average_score = average_score;
    metrics.last_run_id = last_run_id.clone();
    recompute_ratios(&mut metrics);
    let metrics = EvolutionMetrics {
        total_runs: metrics.total_runs,
        passed_runs: metrics.passed_runs,
        failed_runs: metrics.failed_runs,
        real_execution_failed_runs: metrics.real_execution_failed_runs,
        cargo_gate_failed_runs: metrics.cargo_gate_failed_runs,
        replay_failed_runs: metrics.replay_failed_runs,
        safety_rejected_runs: metrics.safety_rejected_runs,
        duplicate_rejected_runs: metrics.duplicate_rejected_runs,
        cosmetic_rejected_runs: metrics.cosmetic_rejected_runs,
        already_promoted_runs: metrics.already_promoted_runs,
        policy_rejected_runs: metrics.policy_rejected_runs,
        unknown_runs: metrics.unknown_runs,
        candidate_count,
        replay_passed,
        replay_failed,
        promoted_count,
        average_score,
        pass_ratio: metrics.pass_ratio,
        effective_failure_ratio: metrics.effective_failure_ratio,
        safety_rejection_ratio: metrics.safety_rejection_ratio,
        last_run_id,
    };
    write_metrics(memory_root, &metrics)?;
    Ok(metrics)
}

pub fn classify_run_outcome(entry: &EvolutionLogEntry) -> EvolutionRunOutcome {
    if entry.duplicate_rejected {
        return EvolutionRunOutcome::DuplicateSafetyRejection;
    }
    if entry.retained_in_core || entry.status == EvolutionStatus::Promoted {
        return EvolutionRunOutcome::AlreadyPromoted;
    }
    if matches!(
        entry.status,
        EvolutionStatus::Passed | EvolutionStatus::Candidate
    ) {
        return EvolutionRunOutcome::Passed;
    }
    if entry.mutation_class == "cosmetic"
        || entry.non_candidate_reason.as_deref() == Some("cosmetic_mutation")
        || entry.mutation_kind == "appendcomment"
    {
        return EvolutionRunOutcome::CosmeticRejection;
    }
    if matches!(
        entry.non_candidate_reason.as_deref(),
        Some("policy_rejection")
            | Some("task_constraints_too_narrow")
            | Some("allowed_targets_filtered_all")
            | Some("allowed_kinds_filtered_all")
            | Some("blocked_by_policy")
    ) {
        return EvolutionRunOutcome::PolicyRejection;
    }
    if entry.status == EvolutionStatus::Failed {
        if !entry.cargo_check_ok || !entry.cargo_test_ok || !entry.cargo_run_ok {
            return EvolutionRunOutcome::CargoGateFailure;
        }
        if entry.non_candidate_reason.as_deref() == Some("blocked_by_operator") {
            return EvolutionRunOutcome::BlockedByOperator;
        }
        if entry.non_candidate_reason.as_deref() == Some("unknown") {
            return EvolutionRunOutcome::Unknown;
        }
        return EvolutionRunOutcome::RealExecutionFailure;
    }
    EvolutionRunOutcome::Unknown
}

pub fn apply_outcome_counts(metrics: &mut EvolutionMetrics, outcome: EvolutionRunOutcome) {
    match outcome {
        EvolutionRunOutcome::Passed => metrics.passed_runs += 1,
        EvolutionRunOutcome::RealExecutionFailure => {
            metrics.failed_runs += 1;
            metrics.real_execution_failed_runs += 1;
        }
        EvolutionRunOutcome::CargoGateFailure => {
            metrics.failed_runs += 1;
            metrics.cargo_gate_failed_runs += 1;
        }
        EvolutionRunOutcome::ReplayFailure => {
            metrics.failed_runs += 1;
            metrics.replay_failed_runs += 1;
        }
        EvolutionRunOutcome::DuplicateSafetyRejection => {
            metrics.safety_rejected_runs += 1;
            metrics.duplicate_rejected_runs += 1;
        }
        EvolutionRunOutcome::CosmeticRejection => {
            metrics.safety_rejected_runs += 1;
            metrics.cosmetic_rejected_runs += 1;
        }
        EvolutionRunOutcome::AlreadyPromoted => metrics.already_promoted_runs += 1,
        EvolutionRunOutcome::PolicyRejection => {
            metrics.safety_rejected_runs += 1;
            metrics.policy_rejected_runs += 1;
        }
        EvolutionRunOutcome::BlockedByOperator => metrics.policy_rejected_runs += 1,
        EvolutionRunOutcome::Unknown => metrics.unknown_runs += 1,
    }
}

fn replay_is_ok(replay: &ReplayResult) -> bool {
    replay.matches_stored_summary
        && replay.replay_status != EvolutionStatus::Failed
        && replay.cargo_check_ok
        && replay.cargo_test_ok
        && replay.cargo_run_ok
}

fn recompute_ratios(metrics: &mut EvolutionMetrics) {
    if metrics.total_runs == 0 {
        metrics.pass_ratio = 0.0;
        metrics.effective_failure_ratio = 0.0;
        metrics.safety_rejection_ratio = 0.0;
        return;
    }
    metrics.pass_ratio = metrics.passed_runs as f32 / metrics.total_runs as f32;
    metrics.effective_failure_ratio = metrics.failed_runs as f32 / metrics.total_runs as f32;
    metrics.safety_rejection_ratio =
        metrics.safety_rejected_runs as f32 / metrics.total_runs as f32;
}

pub fn learning_summary(memory_root: &str) -> Result<String, String> {
    let regressions = regression_memory::load_regressions(memory_root)?;
    let successes = success_memory::load_success_patterns(memory_root)?;
    let dedup_entries = dedup::load_dedup_entries(memory_root)?;

    let risky = top_risky_files(&regressions);
    let successful = top_success_files(&successes);

    Ok(format!(
        "total regression patterns: {}\ntotal success patterns: {}\nmutation dedup count: {}\ntop risky files: {}\ntop successful files: {}\nlearning source count: {}",
        regressions.len(),
        successes.len(),
        dedup_entries.len(),
        risky,
        successful,
        regressions.len() + successes.len()
    ))
}

pub fn write_metrics(memory_root: &str, metrics: &EvolutionMetrics) -> Result<(), String> {
    let path = Path::new(memory_root).join("metrics.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create metrics directory: {error}"))?;
    }
    let contents = serde_json::to_string_pretty(metrics)
        .map_err(|error| format!("failed to serialize metrics: {error}"))?;
    fs::write(path, contents).map_err(|error| format!("failed to write metrics: {error}"))
}

fn load_logs(memory_root: &str) -> Result<Vec<EvolutionLogEntry>, String> {
    let path = Path::new(memory_root).join("evolution.jsonl");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read evolution logs: {error}"))?;
    let mut entries = Vec::new();
    for (index, line) in contents
        .lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
    {
        entries.push(parse_log_line(line, index)?);
    }
    Ok(entries)
}

fn parse_log_line(line: &str, index: usize) -> Result<EvolutionLogEntry, String> {
    if let Ok(entry) = serde_json::from_str::<EvolutionLogEntry>(line) {
        return Ok(entry);
    }
    let legacy: LegacyEvolutionLogEntry = serde_json::from_str(line)
        .map_err(|error| format!("failed to parse evolution log line: {error}"))?;
    Ok(legacy.into_log_entry(index))
}

fn load_replays(memory_root: &str) -> Result<Vec<ReplayResult>, String> {
    let dir = Path::new(memory_root).join("replays");
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut replays = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|error| format!("failed to read replays: {error}"))? {
        let entry = entry.map_err(|error| format!("failed to read replay entry: {error}"))?;
        let contents = fs::read_to_string(entry.path())
            .map_err(|error| format!("failed to read replay file: {error}"))?;
        replays.push(
            serde_json::from_str::<ReplayResult>(&contents)
                .map_err(|error| format!("failed to parse replay file: {error}"))?,
        );
    }
    Ok(replays)
}

fn top_risky_files(entries: &[regression_memory::RegressionEntry]) -> String {
    let mut entries = entries.to_vec();
    entries.sort_by(|left, right| {
        right
            .penalty
            .total_cmp(&left.penalty)
            .then_with(|| left.target_file.cmp(&right.target_file))
    });
    if entries.is_empty() {
        return "(none)".to_string();
    }
    entries
        .iter()
        .take(3)
        .map(|entry| format!("{}:{:.1}", entry.target_file, entry.penalty))
        .collect::<Vec<_>>()
        .join(", ")
}

fn top_success_files(entries: &[success_memory::SuccessPatternEntry]) -> String {
    let mut entries = entries.to_vec();
    entries.sort_by(|left, right| {
        right
            .bonus
            .total_cmp(&left.bonus)
            .then_with(|| left.target_file.cmp(&right.target_file))
    });
    if entries.is_empty() {
        return "(none)".to_string();
    }
    entries
        .iter()
        .take(3)
        .map(|entry| format!("{}:{:.2}", entry.target_file, entry.bonus))
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Debug, Clone, Deserialize)]
struct LegacyEvolutionLogEntry {
    mutation: MutationContract,
    score: LegacyEvolutionScore,
    retained_in_core: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct LegacyEvolutionScore {
    accepted: bool,
    score: f32,
    #[serde(default)]
    useful_change: bool,
    #[serde(default)]
    non_candidate_reason: Option<String>,
    check_passed: bool,
    test_passed: bool,
    run_passed: bool,
}

impl LegacyEvolutionLogEntry {
    fn into_log_entry(self, index: usize) -> EvolutionLogEntry {
        let status = if self.retained_in_core {
            EvolutionStatus::Promoted
        } else if self.score.accepted {
            EvolutionStatus::Passed
        } else {
            EvolutionStatus::Failed
        };

        EvolutionLogEntry {
            run_id: format!("legacy-run-{index}"),
            plan_id: None,
            hypothesis_id: None,
            objective: None,
            graph_evidence: Vec::new(),
            recombined_source_patterns: Vec::new(),
            recombined_avoided_risks: Vec::new(),
            recombination_reason_ru: None,
            portfolio_reason_ru: None,
            selected_strategy: None,
            policy_reason_ru: None,
            mutation_class: crate::evolution::mutation_class_label(
                crate::evolution::classify_mutation_kind_label(
                    &format!("{:?}", self.mutation.kind).to_ascii_lowercase(),
                    self.score.useful_change,
                ),
            )
            .to_string(),
            hygiene_warning_ru: None,
            diversity_bonus: 0.0,
            saturation_penalty: 0.0,
            repeated_target_penalty: 0.0,
            final_recombination_score: 0.0,
            strategy_bonus: 0.0,
            strategy_saturation_penalty: 0.0,
            quality_bonus: 0.0,
            novelty_score: 0.0,
            useful_delta_score: 0.0,
            duplicate_suppression_score: 0.0,
            regression_avoidance_score: 0.0,
            coverage_proxy_score: 0.0,
            quality_score: 0.0,
            final_strategy_score: 0.0,
            mutation_id: self.mutation.id.clone(),
            mutation_digest: String::new(),
            status,
            target_file: self.mutation.target_file.clone(),
            mutation_kind: memory::mutation_kind_label(self.mutation.kind),
            risk: self.mutation.risk,
            score: self.score.score,
            useful_change: self.score.useful_change,
            non_candidate_reason: self.score.non_candidate_reason,
            duplicate_rejected: false,
            regression_penalty: 0.0,
            success_bonus: 0.0,
            cargo_check_ok: self.score.check_passed,
            cargo_test_ok: self.score.test_passed,
            cargo_run_ok: self.score.run_passed,
            retained_in_core: self.retained_in_core,
            sandbox_destroyed: true,
            stdout_digest: String::new(),
            stderr_digest: String::new(),
            stderr_tail: String::new(),
            timestamp_unix: 0,
        }
    }
}
