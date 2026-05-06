use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::contracts::{EvolutionLogEntry, MutationKind};
use crate::evolution::{memory, ReplayResult};

pub const DEFAULT_PORTFOLIO_PATH: &str = "memory/portfolio.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MutationPortfolio {
    #[serde(default)]
    pub kinds: Vec<MutationPortfolioEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MutationPortfolioEntry {
    pub mutation_kind: String,
    pub seen_count: u64,
    pub success_count: u64,
    pub candidate_count: u64,
    pub replay_passed_count: u64,
    pub promoted_count: u64,
    pub average_score: f32,
    pub saturation_score: f32,
    pub last_used_at: u64,
}

impl Default for MutationPortfolioEntry {
    fn default() -> Self {
        Self {
            mutation_kind: String::new(),
            seen_count: 0,
            success_count: 0,
            candidate_count: 0,
            replay_passed_count: 0,
            promoted_count: 0,
            average_score: 0.0,
            saturation_score: 0.0,
            last_used_at: 0,
        }
    }
}

pub fn load_portfolio(memory_root: &str) -> Result<MutationPortfolio, String> {
    let path = Path::new(memory_root).join("portfolio.json");
    if !path.exists() {
        return Ok(MutationPortfolio::default());
    }
    let contents =
        fs::read_to_string(path).map_err(|error| format!("failed to read portfolio: {error}"))?;
    serde_json::from_str(&contents).map_err(|error| format!("failed to parse portfolio: {error}"))
}

pub fn print_portfolio(memory_root: &str) -> Result<String, String> {
    let portfolio = load_portfolio(memory_root)?;
    if portfolio.kinds.is_empty() {
        return Ok("(none)".to_string());
    }
    Ok(portfolio
        .kinds
        .iter()
        .map(|entry| {
            format!(
                "{} seen={} success={} candidates={} replay_passed={} promoted={} avg_score={:.2} saturation={:.2} last_used_at={}",
                entry.mutation_kind,
                entry.seen_count,
                entry.success_count,
                entry.candidate_count,
                entry.replay_passed_count,
                entry.promoted_count,
                entry.average_score,
                entry.saturation_score,
                entry.last_used_at
            )
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

pub fn update_portfolio_after_log(
    memory_root: &str,
    entry: &EvolutionLogEntry,
) -> Result<MutationPortfolio, String> {
    let mut portfolio = load_portfolio(memory_root)?;
    let kind = entry.mutation_kind.to_ascii_lowercase();
    let slot = upsert_entry(&mut portfolio, &kind);
    let previous_seen = slot.seen_count;
    slot.seen_count += 1;
    if entry.cargo_check_ok && entry.cargo_test_ok {
        slot.success_count += 1;
    }
    if entry.status == crate::contracts::EvolutionStatus::Candidate {
        slot.candidate_count += 1;
    }
    if entry.status == crate::contracts::EvolutionStatus::Promoted {
        slot.promoted_count += 1;
    }
    slot.average_score = if previous_seen == 0 {
        entry.score
    } else {
        ((slot.average_score * previous_seen as f32) + entry.score) / slot.seen_count as f32
    };
    slot.last_used_at = entry.timestamp_unix;
    refresh_saturation_scores(&mut portfolio);
    write_portfolio(memory_root, &portfolio)?;
    Ok(portfolio)
}

pub fn update_portfolio_after_replay(
    memory_root: &str,
    mutation_kind: MutationKind,
    replay: &ReplayResult,
) -> Result<MutationPortfolio, String> {
    let mut portfolio = load_portfolio(memory_root)?;
    if replay.matches_stored_summary
        && replay.cargo_check_ok
        && replay.cargo_test_ok
        && replay.cargo_run_ok
        && replay.replay_status != crate::contracts::EvolutionStatus::Failed
    {
        let slot = upsert_entry(&mut portfolio, &kind_label(mutation_kind));
        slot.replay_passed_count += 1;
        slot.last_used_at = replay.timestamp_unix;
    }
    refresh_saturation_scores(&mut portfolio);
    write_portfolio(memory_root, &portfolio)?;
    Ok(portfolio)
}

fn write_portfolio(memory_root: &str, portfolio: &MutationPortfolio) -> Result<(), String> {
    let path = Path::new(memory_root).join("portfolio.json");
    memory::write_json(path, portfolio)
}

fn upsert_entry<'a>(
    portfolio: &'a mut MutationPortfolio,
    mutation_kind: &str,
) -> &'a mut MutationPortfolioEntry {
    if let Some(index) = portfolio
        .kinds
        .iter()
        .position(|entry| entry.mutation_kind == mutation_kind)
    {
        return &mut portfolio.kinds[index];
    }
    portfolio.kinds.push(MutationPortfolioEntry {
        mutation_kind: mutation_kind.to_string(),
        ..MutationPortfolioEntry::default()
    });
    portfolio
        .kinds
        .sort_by(|left, right| left.mutation_kind.cmp(&right.mutation_kind));
    let index = portfolio
        .kinds
        .iter()
        .position(|entry| entry.mutation_kind == mutation_kind)
        .expect("portfolio entry present");
    &mut portfolio.kinds[index]
}

fn refresh_saturation_scores(portfolio: &mut MutationPortfolio) {
    let total_candidates = portfolio
        .kinds
        .iter()
        .map(|entry| entry.candidate_count)
        .sum::<u64>()
        .max(1);
    for entry in &mut portfolio.kinds {
        let share = entry.candidate_count as f32 / total_candidates as f32;
        entry.saturation_score = if share > 0.6 {
            (share - 0.6).clamp(0.0, 0.4)
        } else {
            0.0
        };
    }
}

pub fn kind_label(kind: MutationKind) -> String {
    format!("{kind:?}").to_ascii_lowercase()
}
