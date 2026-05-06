use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::contracts::{sha256_digest, MutationContract};
use crate::evolution::memory::mutation_kind_label;

pub const DEFAULT_MUTATION_DEDUP_PATH: &str = "memory/mutation_dedup.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DedupEntry {
    pub digest: String,
    pub target_file: String,
    pub mutation_kind: String,
    pub score: f32,
    pub useful_change: bool,
    pub run_id: String,
    pub seen_count: u64,
}

pub fn compute_mutation_digest(mutation: &MutationContract) -> String {
    let payload = format!(
        "{}|{}|{}|{}|{}",
        mutation.target_file,
        mutation_kind_label(mutation.kind),
        mutation.search.as_deref().unwrap_or(""),
        mutation.replace.as_deref().unwrap_or(""),
        mutation.append.as_deref().unwrap_or("")
    );
    sha256_digest(&payload)
}

pub fn load_dedup_entries(memory_root: &str) -> Result<Vec<DedupEntry>, String> {
    let path = Path::new(memory_root).join("mutation_dedup.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read dedup memory: {error}"))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("failed to parse dedup memory: {error}"))
}

pub fn should_reject_duplicate_bad(memory_root: &str, digest: &str) -> Result<bool, String> {
    let entries = load_dedup_entries(memory_root)?;
    Ok(entries
        .iter()
        .any(|entry| entry.digest == digest && (!entry.useful_change || entry.score < 5.0)))
}

pub fn record_dedup_entry(
    memory_root: &str,
    digest: &str,
    mutation: &MutationContract,
    score: f32,
    useful_change: bool,
    run_id: &str,
) -> Result<DedupEntry, String> {
    let mut entries = load_dedup_entries(memory_root)?;
    let updated = if let Some(existing) = entries.iter_mut().find(|entry| entry.digest == digest) {
        existing.score = score;
        existing.useful_change = useful_change;
        existing.run_id = run_id.to_string();
        existing.seen_count += 1;
        existing.clone()
    } else {
        let created = DedupEntry {
            digest: digest.to_string(),
            target_file: mutation.target_file.clone(),
            mutation_kind: mutation_kind_label(mutation.kind),
            score,
            useful_change,
            run_id: run_id.to_string(),
            seen_count: 1,
        };
        entries.push(created.clone());
        created
    };
    entries.sort_by(|left, right| left.digest.cmp(&right.digest));
    write_entries(memory_root, &entries)?;
    Ok(updated)
}

fn write_entries(memory_root: &str, entries: &[DedupEntry]) -> Result<(), String> {
    let path = Path::new(memory_root).join("mutation_dedup.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create dedup directory: {error}"))?;
    }
    let contents = serde_json::to_string_pretty(entries)
        .map_err(|error| format!("failed to serialize dedup entries: {error}"))?;
    fs::write(path, contents).map_err(|error| format!("failed to write dedup entries: {error}"))
}
