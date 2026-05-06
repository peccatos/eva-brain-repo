use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::evolution::benchmark::count_sandbox_leaks;
use crate::evolution::load_metrics;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutonomyStatus {
    pub current_level: u8,
    pub allowed_next_level: u8,
    pub blockers: Vec<String>,
    pub required_metrics: Vec<String>,
    pub campaign_mode_allowed: bool,
    pub max_campaign_cycles: usize,
    pub level4_blockers: Vec<String>,
    pub current_safe_autonomy_level: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecentGateSummary {
    recent_checked_runs: usize,
    recent_gate_passed: usize,
    recent_gate_failed: usize,
    stable: bool,
}

pub fn autonomy_status(project_root: &str, memory_root: &str) -> Result<AutonomyStatus, String> {
    let metrics = load_metrics(memory_root)?;
    let sandbox_leaks = count_sandbox_leaks(project_root)?;
    let forbidden_mutations = count_forbidden_mutations(memory_root)?;
    let replay_passed = metrics.replay_passed;
    let regression_rate = regression_rate(memory_root, metrics.total_runs)?;
    let recent_gates =
        recent_gate_summary(memory_root, &metrics, sandbox_leaks, forbidden_mutations)?;
    let useful_candidates = useful_candidate_runs(memory_root)?;
    let pass_ratio = if metrics.total_runs == 0 {
        0.0
    } else {
        metrics.passed_runs as f32 / metrics.total_runs as f32
    };

    let level2_ready = metrics.total_runs >= 10
        && pass_ratio >= 0.8
        && sandbox_leaks == 0
        && forbidden_mutations == 0;
    let level3_ready = level2_ready
        && replay_passed >= 3
        && (metrics.promoted_count >= 1 || useful_candidates >= 1)
        && regression_rate <= 0.3;

    let current_level = if level3_ready {
        3
    } else if level2_ready {
        2
    } else if metrics.total_runs > 0 {
        1
    } else {
        0
    };

    let mut blockers = Vec::new();
    if metrics.total_runs < 10 {
        blockers.push("нужно не менее 10 запусков".to_string());
    }
    if pass_ratio < 0.8 && metrics.total_runs > 0 {
        blockers.push(format!("pass ratio below threshold: {:.2}", pass_ratio));
    }
    if sandbox_leaks > 0 {
        blockers.push(format!("обнаружены утечки sandbox: {sandbox_leaks}"));
    }
    if forbidden_mutations > 0 {
        blockers.push(format!(
            "обнаружены forbidden mutations: {forbidden_mutations}"
        ));
    }
    if current_level < 3 && replay_passed < 3 {
        blockers.push(format!("нужно не менее 3 replay_passed: {replay_passed}"));
    }
    if current_level < 3 && metrics.promoted_count == 0 && useful_candidates == 0 {
        blockers.push("нет promoted_count >= 1 или useful candidates >= 1".to_string());
    }
    if current_level < 3 && regression_rate > 0.3 && metrics.total_runs > 0 {
        blockers.push(format!(
            "слишком высокий regression rate: {:.2}",
            regression_rate
        ));
    }
    if !recent_gates.stable {
        blockers.push(format!(
            "последние sandbox cargo-gates нестабильны: recent_checked_runs={} recent_gate_passed={} recent_gate_failed={}",
            recent_gates.recent_checked_runs,
            recent_gates.recent_gate_passed,
            recent_gates.recent_gate_failed
        ));
    }

    let campaign_mode_allowed = current_level >= 3;
    let max_campaign_cycles = if metrics.total_runs >= 40 && replay_passed >= 8 {
        50
    } else {
        20
    };
    let mut level4_blockers = vec![
        "external ingestion mode is not enabled in this phase".to_string(),
        "auto-promotion remains forbidden".to_string(),
    ];
    if replay_passed < 5 {
        level4_blockers.push(format!("нужно не менее 5 replay_passed: {replay_passed}"));
    }
    if sandbox_leaks > 0 {
        level4_blockers.push("sandbox cleanup нестабилен".to_string());
    }
    if forbidden_mutations > 0 {
        level4_blockers.push("forbidden mutations must stay at zero".to_string());
    }

    Ok(AutonomyStatus {
        current_level,
        allowed_next_level: if current_level >= 3 {
            3
        } else {
            current_level + 1
        },
        blockers,
        required_metrics: vec![
            format!("total_runs={}", metrics.total_runs),
            format!("passed_runs={}", metrics.passed_runs),
            format!("failed_runs={}", metrics.failed_runs),
            format!("replay_passed={replay_passed}"),
            format!("promoted_count={}", metrics.promoted_count),
            format!("useful_candidate_runs={useful_candidates}"),
            format!("sandbox_leaks={sandbox_leaks}"),
            format!("forbidden_mutations={forbidden_mutations}"),
            format!("regression_rate={regression_rate:.2}"),
            format!("recent_checked_runs={}", recent_gates.recent_checked_runs),
            format!("recent_gate_passed={}", recent_gates.recent_gate_passed),
            format!("recent_gate_failed={}", recent_gates.recent_gate_failed),
        ],
        campaign_mode_allowed,
        max_campaign_cycles,
        level4_blockers,
        current_safe_autonomy_level: current_level,
    })
}

fn count_forbidden_mutations(memory_root: &str) -> Result<u64, String> {
    let path = Path::new(memory_root).join("evolution.jsonl");
    if !path.exists() {
        return Ok(0);
    }
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read evolution log: {error}"))?;
    Ok(contents
        .lines()
        .filter_map(parse_explicit_log_entry)
        .filter(|entry| {
            entry.target_file.starts_with("src/core/")
                || entry.target_file == "src/main.rs"
                || entry.target_file == "src/lib.rs"
                || entry.target_file == "Cargo.toml"
        })
        .count() as u64)
}

fn recent_gate_summary(
    memory_root: &str,
    metrics: &crate::evolution::EvolutionMetrics,
    sandbox_leaks: u64,
    forbidden_mutations: u64,
) -> Result<RecentGateSummary, String> {
    let path = Path::new(memory_root).join("evolution.jsonl");
    if !path.exists() {
        return Ok(fallback_recent_gate_summary(
            metrics,
            sandbox_leaks,
            forbidden_mutations,
        ));
    }
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read evolution log: {error}"))?;
    let recent = contents
        .lines()
        .rev()
        .filter_map(parse_explicit_log_entry)
        .filter(is_actual_sandbox_gate_run)
        .take(10)
        .collect::<Vec<_>>();
    if recent.is_empty() {
        return Ok(fallback_recent_gate_summary(
            metrics,
            sandbox_leaks,
            forbidden_mutations,
        ));
    }

    let recent_checked_runs = recent.len();
    let recent_gate_passed = recent
        .iter()
        .filter(|entry| entry.cargo_check_ok && entry.cargo_test_ok && entry.cargo_run_ok)
        .count();
    let recent_gate_failed = recent_checked_runs.saturating_sub(recent_gate_passed);
    let stable =
        recent_gate_failed == 0 || (recent_gate_passed as f32 / recent_checked_runs as f32) >= 0.9;

    Ok(RecentGateSummary {
        recent_checked_runs,
        recent_gate_passed,
        recent_gate_failed,
        stable,
    })
}

fn fallback_recent_gate_summary(
    metrics: &crate::evolution::EvolutionMetrics,
    sandbox_leaks: u64,
    forbidden_mutations: u64,
) -> RecentGateSummary {
    let stable = metrics.passed_runs > 0
        && metrics.failed_runs == 0
        && sandbox_leaks == 0
        && forbidden_mutations == 0;
    RecentGateSummary {
        recent_checked_runs: 0,
        recent_gate_passed: if stable { 1 } else { 0 },
        recent_gate_failed: if stable { 0 } else { 1 },
        stable,
    }
}

fn parse_explicit_log_entry(line: &str) -> Option<crate::EvolutionLogEntry> {
    let entry = serde_json::from_str::<crate::EvolutionLogEntry>(line).ok()?;
    if entry.run_id.starts_with("legacy-run-") || entry.timestamp_unix == 0 {
        return None;
    }
    Some(entry)
}

fn is_actual_sandbox_gate_run(entry: &crate::EvolutionLogEntry) -> bool {
    if entry.duplicate_rejected {
        return false;
    }
    if entry.retained_in_core {
        return false;
    }
    if entry.mutation_kind == "appendcomment"
        || entry.non_candidate_reason.as_deref() == Some("cosmetic_mutation")
    {
        return false;
    }
    true
}

fn regression_rate(memory_root: &str, total_runs: u64) -> Result<f32, String> {
    if total_runs == 0 {
        return Ok(0.0);
    }
    let entries = crate::evolution::load_regressions(memory_root)?;
    Ok(entries.len() as f32 / total_runs as f32)
}

fn useful_candidate_runs(memory_root: &str) -> Result<u64, String> {
    let path = Path::new(memory_root).join("evolution.jsonl");
    if !path.exists() {
        return Ok(0);
    }
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read evolution log: {error}"))?;
    Ok(contents
        .lines()
        .filter_map(parse_explicit_log_entry)
        .filter(|entry| entry.status == crate::contracts::EvolutionStatus::Candidate)
        .count() as u64)
}
