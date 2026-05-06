use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::contracts::EvolutionLogEntry;

pub const DEFAULT_REGRESSION_MEMORY_PATH: &str = "memory/regressions.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegressionEntry {
    pub pattern_id: String,
    pub target_area: String,
    pub target_file: String,
    pub mutation_kind: String,
    pub failure_status: String,
    pub fail_count: u64,
    pub penalty: f32,
    pub last_run_id: String,
}

pub fn load_regressions(memory_root: &str) -> Result<Vec<RegressionEntry>, String> {
    let path = Path::new(memory_root).join("regressions.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read regressions: {error}"))?;
    serde_json::from_str(&contents).map_err(|error| format!("failed to parse regressions: {error}"))
}

pub fn record_regression(
    memory_root: &str,
    entry: &EvolutionLogEntry,
) -> Result<RegressionEntry, String> {
    let mut regressions = load_regressions(memory_root)?;
    let pattern_id = pattern_id(entry);
    let target_area = target_area(&entry.target_file);
    let failure_status = if entry.duplicate_rejected {
        "duplicate_rejected".to_string()
    } else if entry.useful_change {
        "failed".to_string()
    } else {
        "non_useful".to_string()
    };

    let updated = if let Some(existing) = regressions.iter_mut().find(|existing| {
        existing.pattern_id == pattern_id && existing.target_file == entry.target_file
    }) {
        existing.fail_count += 1;
        existing.penalty = (existing.fail_count as f32 * 0.5).min(5.0);
        existing.failure_status = failure_status.clone();
        existing.last_run_id = entry.run_id.clone();
        existing.clone()
    } else {
        let created = RegressionEntry {
            pattern_id,
            target_area,
            target_file: entry.target_file.clone(),
            mutation_kind: entry.mutation_kind.clone(),
            failure_status,
            fail_count: 1,
            penalty: 0.5,
            last_run_id: entry.run_id.clone(),
        };
        regressions.push(created.clone());
        created
    };

    regressions.sort_by(|left, right| left.pattern_id.cmp(&right.pattern_id));
    write_regressions(memory_root, &regressions)?;
    Ok(updated)
}

fn write_regressions(memory_root: &str, entries: &[RegressionEntry]) -> Result<(), String> {
    let path = Path::new(memory_root).join("regressions.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create regressions directory: {error}"))?;
    }
    let contents = serde_json::to_string_pretty(entries)
        .map_err(|error| format!("failed to serialize regressions: {error}"))?;
    fs::write(path, contents).map_err(|error| format!("failed to write regressions: {error}"))
}

pub fn pattern_id(entry: &EvolutionLogEntry) -> String {
    if let Some(plan_id) = &entry.plan_id {
        plan_id.clone()
    } else {
        format!("pattern:{}:{}", entry.mutation_kind, entry.target_file)
    }
}

pub fn target_area(target_file: &str) -> String {
    Path::new(target_file)
        .parent()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_else(|| target_file.to_string())
}
