use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

use crate::contracts::EvolutionLogEntry;
use crate::evolution::{load_regressions, load_success_patterns};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DistilledPatternSummary {
    pub generated_at: u64,
    pub top_successful_mutation_kinds: Vec<PatternCount>,
    pub risky_target_files: Vec<RiskyFileSummary>,
    pub preferred_objectives: Vec<PatternCount>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatternCount {
    pub key: String,
    pub count: u64,
    pub average_score: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiskyFileSummary {
    pub target_file: String,
    pub fail_count: u64,
    pub penalty: f32,
}

pub fn distill_patterns(memory_root: &str) -> Result<DistilledPatternSummary, String> {
    let logs = load_logs(memory_root)?;
    let successes = load_success_patterns(memory_root)?;
    let regressions = load_regressions(memory_root)?;

    let mut mutation_stats: BTreeMap<String, (u64, f32)> = BTreeMap::new();
    for entry in &successes {
        let stat = mutation_stats
            .entry(entry.mutation_kind.clone())
            .or_insert((0, 0.0));
        stat.0 += entry.success_count;
        stat.1 += entry.average_score * entry.success_count as f32;
    }

    let mut objective_stats: BTreeMap<String, (u64, f32)> = BTreeMap::new();
    for log in logs.iter().filter(|entry| entry.useful_change) {
        if let Some(objective) = &log.objective {
            let stat = objective_stats.entry(objective.clone()).or_insert((0, 0.0));
            stat.0 += 1;
            stat.1 += log.score;
        }
    }

    let mut risky_files = regressions
        .into_iter()
        .map(|entry| RiskyFileSummary {
            target_file: entry.target_file,
            fail_count: entry.fail_count,
            penalty: entry.penalty,
        })
        .collect::<Vec<_>>();
    risky_files.sort_by(|left, right| {
        right
            .penalty
            .total_cmp(&left.penalty)
            .then_with(|| right.fail_count.cmp(&left.fail_count))
            .then_with(|| left.target_file.cmp(&right.target_file))
    });
    risky_files.truncate(5);

    let summary = DistilledPatternSummary {
        generated_at: crate::evolution::memory::now_unix(),
        top_successful_mutation_kinds: to_pattern_counts(mutation_stats),
        risky_target_files: risky_files,
        preferred_objectives: to_pattern_counts(objective_stats),
    };

    crate::evolution::memory::write_json(
        Path::new(memory_root)
            .join("patterns")
            .join("local_distilled_patterns.json"),
        &summary,
    )?;
    Ok(summary)
}

fn to_pattern_counts(map: BTreeMap<String, (u64, f32)>) -> Vec<PatternCount> {
    let mut values = map
        .into_iter()
        .map(|(key, (count, total_score))| PatternCount {
            key,
            count,
            average_score: if count == 0 {
                0.0
            } else {
                total_score / count as f32
            },
        })
        .collect::<Vec<_>>();
    values.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| right.average_score.total_cmp(&left.average_score))
            .then_with(|| left.key.cmp(&right.key))
    });
    values.truncate(5);
    values
}

fn load_logs(memory_root: &str) -> Result<Vec<EvolutionLogEntry>, String> {
    let path = Path::new(memory_root).join("evolution.jsonl");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read evolution log: {error}"))?;
    Ok(contents
        .lines()
        .filter_map(|line| serde_json::from_str::<EvolutionLogEntry>(line).ok())
        .collect())
}
