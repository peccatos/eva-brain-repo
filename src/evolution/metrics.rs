use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::contracts::{EvolutionLogEntry, EvolutionStatus, MutationContract};
use crate::evolution::memory::ReplayResult;
use crate::evolution::{dedup, memory, regression_memory, success_memory};

pub const DEFAULT_METRICS_PATH: &str = "memory/metrics.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct EvolutionMetrics {
    pub total_runs: u64,
    pub passed_runs: u64,
    pub failed_runs: u64,
    pub candidate_count: u64,
    pub replay_passed: u64,
    pub promoted_count: u64,
    pub average_score: f32,
    pub last_run_id: Option<String>,
}

pub fn load_metrics(memory_root: &str) -> Result<EvolutionMetrics, String> {
    refresh_metrics(memory_root)
}

fn load_cached_metrics(memory_root: &str) -> Result<EvolutionMetrics, String> {
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
    let mut metrics = load_cached_metrics(memory_root)?;
    let previous_total = metrics.total_runs;
    metrics.total_runs += 1;
    match entry.status {
        EvolutionStatus::Failed => metrics.failed_runs += 1,
        EvolutionStatus::Candidate => {
            metrics.passed_runs += 1;
            metrics.candidate_count += 1;
        }
        EvolutionStatus::Promoted => {
            metrics.passed_runs += 1;
            metrics.promoted_count += 1;
        }
        EvolutionStatus::Passed => metrics.passed_runs += 1,
    }
    metrics.average_score =
        ((metrics.average_score * previous_total as f32) + entry.score) / metrics.total_runs as f32;
    metrics.last_run_id = Some(entry.run_id.clone());
    write_metrics(memory_root, &metrics)?;
    Ok(metrics)
}

pub fn update_metrics_after_replay(
    memory_root: &str,
    replay: &ReplayResult,
) -> Result<EvolutionMetrics, String> {
    let mut metrics = load_cached_metrics(memory_root)?;
    if replay.matches_stored_summary && replay.cargo_check_ok && replay.cargo_test_ok {
        metrics.replay_passed += 1;
    }
    write_metrics(memory_root, &metrics)?;
    Ok(metrics)
}

pub fn refresh_metrics(memory_root: &str) -> Result<EvolutionMetrics, String> {
    let logs = load_logs(memory_root)?;
    let summaries = memory::list_candidate_summaries(memory_root)?;
    let replays = load_replays(memory_root)?;

    let total_runs = logs.len() as u64;
    let passed_runs = logs
        .iter()
        .filter(|entry| entry.status != EvolutionStatus::Failed)
        .count() as u64;
    let failed_runs = logs
        .iter()
        .filter(|entry| entry.status == EvolutionStatus::Failed)
        .count() as u64;
    let candidate_count = summaries.len() as u64;
    let replay_passed = replays
        .iter()
        .filter(|replay| {
            replay.matches_stored_summary && replay.replay_status != EvolutionStatus::Failed
        })
        .count() as u64;
    let promoted_count = logs.iter().filter(|entry| entry.retained_in_core).count() as u64;
    let average_score = if logs.is_empty() {
        0.0
    } else {
        logs.iter().map(|entry| entry.score).sum::<f32>() / logs.len() as f32
    };
    let last_run_id = logs.last().map(|entry| entry.run_id.clone());
    let metrics = EvolutionMetrics {
        total_runs,
        passed_runs,
        failed_runs,
        candidate_count,
        replay_passed,
        promoted_count,
        average_score,
        last_run_id,
    };
    write_metrics(memory_root, &metrics)?;
    Ok(metrics)
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
            diversity_bonus: 0.0,
            saturation_penalty: 0.0,
            repeated_target_penalty: 0.0,
            final_recombination_score: 0.0,
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
