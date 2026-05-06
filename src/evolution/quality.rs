use serde::{Deserialize, Serialize};

use crate::contracts::EvolutionLogEntry;
use crate::evolution::{load_portfolio, load_regressions, load_strategy_portfolio};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct QualityMetricsV2 {
    pub novelty_score: f32,
    pub useful_delta_score: f32,
    pub duplicate_suppression_score: f32,
    pub regression_avoidance_score: f32,
    pub coverage_proxy_score: f32,
    pub quality_score: f32,
}

pub fn compute_quality_for_hypothesis(
    memory_root: &str,
    mutation_kind: &str,
    target_file: &str,
    strategy: &str,
    source_patterns: &[String],
    avoided_risks: &[String],
) -> Result<QualityMetricsV2, String> {
    let portfolio = load_portfolio(memory_root)?;
    let strategy_portfolio = load_strategy_portfolio(memory_root).unwrap_or_default();
    let regressions = load_regressions(memory_root)?;
    let mutation_seen = portfolio
        .kinds
        .iter()
        .find(|entry| entry.mutation_kind == mutation_kind)
        .map(|entry| entry.seen_count)
        .unwrap_or(0);
    let strategy_seen = strategy_portfolio
        .strategies
        .iter()
        .find(|entry| entry.strategy == strategy)
        .map(|entry| entry.seen_count)
        .unwrap_or(0);
    let novelty_score =
        (0.6 - (mutation_seen.min(10) as f32 * 0.04) - (strategy_seen.min(10) as f32 * 0.02))
            .clamp(0.0, 0.6);
    let useful_delta_score = match mutation_kind {
        "addmetricupdate" | "addlearningsummaryfield" => 0.28,
        "addreplayassertion" => 0.26,
        "addunittest" => 0.20,
        _ => 0.10,
    };
    let duplicate_suppression_score = if source_patterns
        .iter()
        .any(|pattern| pattern.starts_with("success_kind:"))
    {
        0.10
    } else {
        0.20
    };
    let regression_avoidance_score = if avoided_risks
        .iter()
        .any(|risk| risk.contains("runtime_to_tests_redirect"))
        || regressions
            .iter()
            .any(|entry| entry.target_file == target_file)
    {
        0.30
    } else {
        0.10
    };
    let coverage_proxy_score = if target_file.starts_with("tests/")
        || mutation_kind == "addreplayassertion"
        || strategy == "ReplaySafety"
    {
        0.25
    } else {
        0.12
    };
    let quality_score = (novelty_score
        + useful_delta_score
        + duplicate_suppression_score
        + regression_avoidance_score
        + coverage_proxy_score)
        .clamp(0.0, 2.0);
    Ok(QualityMetricsV2 {
        novelty_score,
        useful_delta_score,
        duplicate_suppression_score,
        regression_avoidance_score,
        coverage_proxy_score,
        quality_score,
    })
}

pub fn compute_quality_for_run(
    memory_root: &str,
    run_id: &str,
) -> Result<QualityMetricsV2, String> {
    let path = std::path::Path::new(memory_root).join("evolution.jsonl");
    let contents = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read evolution log: {error}"))?;
    let entry = contents
        .lines()
        .filter_map(|line| serde_json::from_str::<EvolutionLogEntry>(line).ok())
        .find(|entry| entry.run_id == run_id)
        .ok_or_else(|| format!("run not found for quality report: {run_id}"))?;
    let strategy = entry
        .selected_strategy
        .clone()
        .unwrap_or_else(|| "TestExpansion".to_string());
    compute_quality_for_hypothesis(
        memory_root,
        &entry.mutation_kind,
        &entry.target_file,
        &strategy,
        &entry.recombined_source_patterns,
        &entry.recombined_avoided_risks,
    )
}

pub fn print_quality_report(memory_root: &str, run_id: &str) -> Result<String, String> {
    let quality = compute_quality_for_run(memory_root, run_id)?;
    Ok(serde_json::to_string_pretty(&quality).expect("serialize quality report"))
}
