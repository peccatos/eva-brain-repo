use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::contracts::DeterminismAuditReport;
use crate::evolution::{build_release_health, memory};

pub fn build_determinism_audit(
    project_root: &str,
    memory_root: &str,
) -> Result<DeterminismAuditReport, String> {
    let mut checked_documents = Vec::new();
    let mut missing_required_fields = Vec::new();
    let mut unstable_field_warnings = Vec::new();
    let mut full_source_content_warnings = Vec::new();

    inspect_json_dir(
        Path::new(memory_root).join("releases").join("manifests"),
        &[
            "release_id",
            "source_run_id",
            "target_file",
            "mutation_kind",
            "mutation_class",
            "replay_status",
            "approved",
            "auto_promote",
            "source_mutated",
            "rollback_available",
            "changelog_available",
            "created_at",
        ],
        &mut checked_documents,
        &mut missing_required_fields,
        &mut unstable_field_warnings,
        &mut full_source_content_warnings,
    )?;
    inspect_json_dir(
        Path::new(memory_root).join("releases").join("rollback"),
        &[
            "release_id",
            "source_run_id",
            "target_file",
            "rollback_type",
            "rollback_available",
            "original_candidate_report_path",
            "notes",
            "created_at",
        ],
        &mut checked_documents,
        &mut missing_required_fields,
        &mut unstable_field_warnings,
        &mut full_source_content_warnings,
    )?;
    inspect_json_file(
        Path::new(memory_root).join("proof").join("eva_proof.json"),
        &[
            "generated_at",
            "release_runtime_support",
            "auto_promote",
            "operator_approval_required",
        ],
        &mut checked_documents,
        &mut missing_required_fields,
        &mut unstable_field_warnings,
        &mut full_source_content_warnings,
    )?;

    let release_health = serde_json::to_value(build_release_health(project_root, memory_root)?)
        .map_err(|error| format!("failed to encode release health for audit: {error}"))?;
    checked_documents.push("release_health:runtime".to_string());
    for field in [
        "generated_at",
        "release_runtime_support",
        "release_count",
        "candidate_count",
        "auto_promote",
        "operator_approval_required",
        "health_score",
        "health_grade",
    ] {
        if release_health.get(field).is_none() {
            missing_required_fields.push(format!("release_health:runtime:{field}"));
        }
    }
    inspect_value_for_source_markers(
        "release_health:runtime",
        &release_health,
        &mut full_source_content_warnings,
    );

    let deterministic_enough =
        missing_required_fields.is_empty() && full_source_content_warnings.is_empty();
    let recommendations_ru = if deterministic_enough {
        vec!["Структура release/proof артефактов достаточно детерминирована.".to_string()]
    } else {
        vec!["Исправить missing/full-source предупреждения перед release gate.".to_string()]
    };

    missing_required_fields.sort();
    missing_required_fields.dedup();
    unstable_field_warnings.sort();
    unstable_field_warnings.dedup();
    full_source_content_warnings.sort();
    full_source_content_warnings.dedup();
    checked_documents.sort();
    checked_documents.dedup();

    Ok(DeterminismAuditReport {
        generated_at: memory::now_unix(),
        checked_documents,
        missing_required_fields,
        unstable_field_warnings,
        full_source_content_warnings,
        deterministic_enough,
        recommendations_ru,
    })
}

pub fn print_determinism_audit(project_root: &str, memory_root: &str) -> Result<String, String> {
    let report = build_determinism_audit(project_root, memory_root)?;
    Ok(format!(
        "determinism_audit: checked={} missing={} full_source_warnings={} deterministic_enough={}\nrecommendations={}",
        report.checked_documents.len(),
        report.missing_required_fields.len(),
        report.full_source_content_warnings.len(),
        report.deterministic_enough,
        report.recommendations_ru.join("; ")
    ))
}

pub fn print_determinism_audit_json(
    project_root: &str,
    memory_root: &str,
) -> Result<String, String> {
    serde_json::to_string_pretty(&build_determinism_audit(project_root, memory_root)?)
        .map_err(|error| format!("failed to serialize determinism audit: {error}"))
}

fn inspect_json_dir(
    dir: PathBuf,
    required_fields: &[&str],
    checked_documents: &mut Vec<String>,
    missing_required_fields: &mut Vec<String>,
    unstable_field_warnings: &mut Vec<String>,
    full_source_content_warnings: &mut Vec<String>,
) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    let mut files = fs::read_dir(&dir)
        .map_err(|error| format!("failed to read determinism audit dir: {error}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect::<Vec<_>>();
    files.sort();
    for path in files {
        inspect_json_file(
            path,
            required_fields,
            checked_documents,
            missing_required_fields,
            unstable_field_warnings,
            full_source_content_warnings,
        )?;
    }
    Ok(())
}

fn inspect_json_file(
    path: PathBuf,
    required_fields: &[&str],
    checked_documents: &mut Vec<String>,
    missing_required_fields: &mut Vec<String>,
    unstable_field_warnings: &mut Vec<String>,
    full_source_content_warnings: &mut Vec<String>,
) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let label = path.display().to_string();
    let contents =
        fs::read_to_string(&path).map_err(|error| format!("failed to read audit json: {error}"))?;
    let value: Value = serde_json::from_str(&contents)
        .map_err(|error| format!("failed to parse audit json {label}: {error}"))?;
    checked_documents.push(label.clone());
    for field in required_fields {
        if value.get(*field).is_none() {
            missing_required_fields.push(format!("{label}:{field}"));
        }
    }
    if value.get("created_at").is_none() && value.get("generated_at").is_none() {
        unstable_field_warnings.push(format!("{label}:missing_timestamp"));
    }
    inspect_value_for_source_markers(&label, &value, full_source_content_warnings);
    Ok(())
}

fn inspect_value_for_source_markers(label: &str, value: &Value, warnings: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            let lower = text.to_ascii_lowercase();
            if lower.contains("fn main(")
                || lower.contains("pub fn ")
                || lower.contains("impl ")
                || lower.contains("use std::")
            {
                warnings.push(format!("{label}:possible_full_source_content"));
            }
            if lower.contains("http://")
                || lower.contains("https://")
                || lower.contains("github.com")
            {
                warnings.push(format!("{label}:network_url_marker"));
            }
        }
        Value::Array(items) => {
            for item in items {
                inspect_value_for_source_markers(label, item, warnings);
            }
        }
        Value::Object(map) => {
            for item in map.values() {
                inspect_value_for_source_markers(label, item, warnings);
            }
        }
        _ => {}
    }
}
