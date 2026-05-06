use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::contracts::EvolutionLogEntry;
use crate::evolution::regression_memory::{pattern_id, target_area};

pub const DEFAULT_SUCCESS_MEMORY_PATH: &str = "memory/success_patterns.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SuccessPatternEntry {
    pub pattern_id: String,
    pub target_area: String,
    pub target_file: String,
    pub mutation_kind: String,
    pub success_count: u64,
    pub average_score: f32,
    pub bonus: f32,
    pub last_run_id: String,
}

pub fn load_success_patterns(memory_root: &str) -> Result<Vec<SuccessPatternEntry>, String> {
    let path = Path::new(memory_root).join("success_patterns.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read success patterns: {error}"))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("failed to parse success patterns: {error}"))
}

pub fn record_success_pattern(
    memory_root: &str,
    entry: &EvolutionLogEntry,
) -> Result<SuccessPatternEntry, String> {
    let mut patterns = load_success_patterns(memory_root)?;
    let pattern_key = pattern_id(entry);
    let area = target_area(&entry.target_file);

    let updated = if let Some(existing) = patterns.iter_mut().find(|existing| {
        existing.pattern_id == pattern_key && existing.target_file == entry.target_file
    }) {
        let previous_count = existing.success_count;
        existing.success_count += 1;
        existing.average_score = ((existing.average_score * previous_count as f32) + entry.score)
            / existing.success_count as f32;
        existing.bonus = (existing.success_count as f32 * 0.25).min(3.0);
        existing.last_run_id = entry.run_id.clone();
        existing.clone()
    } else {
        let created = SuccessPatternEntry {
            pattern_id: pattern_key,
            target_area: area,
            target_file: entry.target_file.clone(),
            mutation_kind: entry.mutation_kind.clone(),
            success_count: 1,
            average_score: entry.score,
            bonus: 0.25,
            last_run_id: entry.run_id.clone(),
        };
        patterns.push(created.clone());
        created
    };

    patterns.sort_by(|left, right| left.pattern_id.cmp(&right.pattern_id));
    write_success_patterns(memory_root, &patterns)?;
    Ok(updated)
}

fn write_success_patterns(
    memory_root: &str,
    entries: &[SuccessPatternEntry],
) -> Result<(), String> {
    let path = Path::new(memory_root).join("success_patterns.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create success patterns directory: {error}"))?;
    }
    let contents = serde_json::to_string_pretty(entries)
        .map_err(|error| format!("failed to serialize success patterns: {error}"))?;
    fs::write(path, contents).map_err(|error| format!("failed to write success patterns: {error}"))
}
