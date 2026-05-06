use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::contracts::EvolutionLogEntry;
use crate::evolution::{memory, ReplayResult};

pub const DEFAULT_STRATEGY_PORTFOLIO_PATH: &str = "memory/strategy_portfolio.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StrategyPortfolio {
    #[serde(default)]
    pub strategies: Vec<StrategyPortfolioEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrategyPortfolioEntry {
    pub strategy: String,
    pub seen_count: u64,
    pub useful_count: u64,
    pub replay_passed_count: u64,
    pub promoted_count: u64,
    pub average_score: f32,
    pub average_risk: f32,
    pub saturation_score: f32,
    pub last_used_at: u64,
}

impl Default for StrategyPortfolioEntry {
    fn default() -> Self {
        Self {
            strategy: String::new(),
            seen_count: 0,
            useful_count: 0,
            replay_passed_count: 0,
            promoted_count: 0,
            average_score: 0.0,
            average_risk: 0.0,
            saturation_score: 0.0,
            last_used_at: 0,
        }
    }
}

pub fn load_strategy_portfolio(memory_root: &str) -> Result<StrategyPortfolio, String> {
    let path = Path::new(memory_root).join("strategy_portfolio.json");
    if !path.exists() {
        return Ok(StrategyPortfolio::default());
    }
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read strategy portfolio: {error}"))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("failed to parse strategy portfolio: {error}"))
}

pub fn print_strategy_portfolio(memory_root: &str) -> Result<String, String> {
    let portfolio = ensure_strategy_portfolio(memory_root)?;
    if portfolio.strategies.is_empty() {
        return Ok("(none)".to_string());
    }
    Ok(portfolio
        .strategies
        .iter()
        .map(|entry| {
            format!(
                "{} seen={} useful={} replay_passed={} promoted={} avg_score={:.2} avg_risk={:.2} saturation={:.2} last_used_at={}",
                entry.strategy,
                entry.seen_count,
                entry.useful_count,
                entry.replay_passed_count,
                entry.promoted_count,
                entry.average_score,
                entry.average_risk,
                entry.saturation_score,
                entry.last_used_at
            )
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

pub fn ensure_strategy_portfolio(memory_root: &str) -> Result<StrategyPortfolio, String> {
    let portfolio = load_strategy_portfolio(memory_root)?;
    if portfolio.strategies.is_empty() {
        return refresh_strategy_portfolio(memory_root);
    }
    Ok(portfolio)
}

pub fn refresh_strategy_portfolio(memory_root: &str) -> Result<StrategyPortfolio, String> {
    let mut portfolio = StrategyPortfolio::default();
    let logs = load_logs(memory_root)?;
    for entry in &logs {
        let strategy = infer_strategy(
            entry.selected_strategy.as_deref(),
            entry.objective.as_deref(),
            &entry.mutation_kind,
            &entry.target_file,
            &entry.recombined_avoided_risks,
        );
        let slot = upsert_strategy(&mut portfolio, &strategy);
        let previous_seen = slot.seen_count;
        slot.seen_count += 1;
        if entry.useful_change {
            slot.useful_count += 1;
        }
        if entry.retained_in_core {
            slot.promoted_count += 1;
        }
        slot.average_score = if previous_seen == 0 {
            entry.score
        } else {
            ((slot.average_score * previous_seen as f32) + entry.score) / slot.seen_count as f32
        };
        slot.average_risk = if previous_seen == 0 {
            entry.risk
        } else {
            ((slot.average_risk * previous_seen as f32) + entry.risk) / slot.seen_count as f32
        };
        slot.last_used_at = entry.timestamp_unix;
    }

    let replay_dir = Path::new(memory_root).join("replays");
    if replay_dir.exists() {
        let mut replay_paths = fs::read_dir(&replay_dir)
            .map_err(|error| format!("failed to read replays: {error}"))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        replay_paths.sort();
        for path in replay_paths {
            let contents = fs::read_to_string(&path)
                .map_err(|error| format!("failed to read replay file: {error}"))?;
            let replay: ReplayResult = serde_json::from_str(&contents)
                .map_err(|error| format!("failed to parse replay file: {error}"))?;
            if replay.matches_stored_summary
                && replay.cargo_check_ok
                && replay.cargo_test_ok
                && replay.cargo_run_ok
                && replay.replay_status != crate::contracts::EvolutionStatus::Failed
            {
                let run_id = path
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default();
                if let Ok(mutation) = memory::load_candidate(memory_root, run_id) {
                    let strategy = infer_strategy(
                        None,
                        None,
                        &mutation_kind_label(mutation.kind),
                        &mutation.target_file,
                        &[],
                    );
                    let slot = upsert_strategy(&mut portfolio, &strategy);
                    slot.replay_passed_count += 1;
                    slot.last_used_at = slot.last_used_at.max(replay.timestamp_unix);
                }
            }
        }
    }

    refresh_saturation_scores(&mut portfolio);
    write_strategy_portfolio(memory_root, &portfolio)?;
    Ok(portfolio)
}

pub fn infer_strategy(
    selected_strategy: Option<&str>,
    objective: Option<&str>,
    mutation_kind: &str,
    target_file: &str,
    avoided_risks: &[String],
) -> String {
    if let Some(strategy) = selected_strategy {
        return strategy.to_string();
    }
    if target_file.contains("review") || target_file.contains("promotion") {
        return "CandidateReview".to_string();
    }
    if avoided_risks
        .iter()
        .any(|risk| risk.contains("runtime_to_tests_redirect") || risk.contains("regression"))
    {
        return "RegressionAvoidance".to_string();
    }
    if objective == Some("ImproveValidation") || target_file.contains("validator") {
        return "ValidationHardening".to_string();
    }
    if mutation_kind == "addreplayassertion"
        || objective == Some("ImproveReplayability")
        || target_file.contains("replay")
    {
        return "ReplaySafety".to_string();
    }
    if mutation_kind == "addmetricupdate"
        || mutation_kind == "addlearningsummaryfield"
        || target_file.contains("metrics")
        || target_file.contains("report")
    {
        return "MetricsReporting".to_string();
    }
    if target_file.starts_with("tests/") || mutation_kind == "addunittest" {
        return "TestExpansion".to_string();
    }
    "CandidateReview".to_string()
}

fn load_logs(memory_root: &str) -> Result<Vec<EvolutionLogEntry>, String> {
    let path = Path::new(memory_root).join("evolution.jsonl");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read evolution log: {error}"))?;
    Ok(contents
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<EvolutionLogEntry>(line).ok())
        .collect())
}

fn write_strategy_portfolio(
    memory_root: &str,
    portfolio: &StrategyPortfolio,
) -> Result<(), String> {
    memory::write_json(
        Path::new(memory_root).join("strategy_portfolio.json"),
        portfolio,
    )
}

fn upsert_strategy<'a>(
    portfolio: &'a mut StrategyPortfolio,
    strategy: &str,
) -> &'a mut StrategyPortfolioEntry {
    if let Some(index) = portfolio
        .strategies
        .iter()
        .position(|entry| entry.strategy == strategy)
    {
        return &mut portfolio.strategies[index];
    }
    portfolio.strategies.push(StrategyPortfolioEntry {
        strategy: strategy.to_string(),
        ..StrategyPortfolioEntry::default()
    });
    portfolio
        .strategies
        .sort_by(|left, right| left.strategy.cmp(&right.strategy));
    let index = portfolio
        .strategies
        .iter()
        .position(|entry| entry.strategy == strategy)
        .expect("strategy present");
    &mut portfolio.strategies[index]
}

fn refresh_saturation_scores(portfolio: &mut StrategyPortfolio) {
    let total_useful = portfolio
        .strategies
        .iter()
        .map(|entry| entry.useful_count)
        .sum::<u64>()
        .max(1);
    for entry in &mut portfolio.strategies {
        let share = entry.useful_count as f32 / total_useful as f32;
        entry.saturation_score = if share > 0.6 {
            (share - 0.6).clamp(0.0, 0.4)
        } else {
            0.0
        };
    }
}

fn mutation_kind_label(kind: crate::contracts::MutationKind) -> String {
    format!("{kind:?}").to_ascii_lowercase()
}
