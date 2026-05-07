use std::fs;
use std::path::Path;

use crate::contracts::ReleaseLedgerRecord;
use crate::evolution::{
    build_preflight_gate, build_release_health, latest_release_id, memory, promotion_ready_approved,
};

const LEDGER_PATH: &str = "releases/release_ledger.jsonl";

pub fn record_release_attempt(
    project_root: &str,
    memory_root: &str,
    release_id: &str,
) -> Result<ReleaseLedgerRecord, String> {
    let bundle_path = Path::new(memory_root)
        .join("releases")
        .join("bundles")
        .join(format!("{release_id}.json"));
    if !bundle_path.exists() {
        return Err(format!("release bundle not found for {release_id}"));
    }
    let gate = build_preflight_gate(project_root, memory_root)?;
    let health = build_release_health(project_root, memory_root)?;
    let record = ReleaseLedgerRecord {
        release_id: release_id.to_string(),
        status: if gate.gate_status == "fail" {
            "blocked".to_string()
        } else {
            "recorded".to_string()
        },
        gate_status: gate.gate_status,
        health_grade: health.health_grade,
        approved_candidate_count: promotion_ready_approved(project_root, memory_root)?.len(),
        generated_at: memory::now_unix(),
    };
    memory::append_jsonl(Path::new(memory_root).join(LEDGER_PATH), &record)?;
    Ok(record)
}

pub fn load_release_ledger(memory_root: &str) -> Result<Vec<ReleaseLedgerRecord>, String> {
    let path = Path::new(memory_root).join(LEDGER_PATH);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read release ledger: {error}"))?;
    let mut records = contents
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str::<ReleaseLedgerRecord>(line)
                .map_err(|error| format!("failed to parse release ledger record: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    records.sort_by(|left, right| {
        left.generated_at
            .cmp(&right.generated_at)
            .then_with(|| left.release_id.cmp(&right.release_id))
            .then_with(|| left.status.cmp(&right.status))
    });
    Ok(records)
}

pub fn print_release_ledger(memory_root: &str) -> Result<String, String> {
    let records = load_release_ledger(memory_root)?;
    if records.is_empty() {
        return Ok("release_ledger: empty".to_string());
    }
    Ok(records
        .iter()
        .map(|record| {
            format!(
                "{} status={} gate={} health={} approved_candidates={}",
                record.release_id,
                record.status,
                record.gate_status,
                record.health_grade,
                record.approved_candidate_count
            )
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

pub fn print_release_ledger_json(memory_root: &str) -> Result<String, String> {
    serde_json::to_string_pretty(&load_release_ledger(memory_root)?)
        .map_err(|error| format!("failed to serialize release ledger: {error}"))
}

pub fn print_record_release_attempt(
    project_root: &str,
    memory_root: &str,
    release_id: &str,
) -> Result<String, String> {
    let record = record_release_attempt(project_root, memory_root, release_id)?;
    serde_json::to_string_pretty(&record)
        .map_err(|error| format!("failed to serialize release ledger record: {error}"))
}

pub fn release_ledger_count(memory_root: &str) -> Result<usize, String> {
    Ok(load_release_ledger(memory_root)?.len())
}

pub fn latest_release_or_none(memory_root: &str) -> Result<String, String> {
    Ok(latest_release_id(memory_root)?.unwrap_or_else(|| "none".to_string()))
}
