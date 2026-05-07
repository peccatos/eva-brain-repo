use crate::contracts::PreflightGateReport;
use crate::evolution::{
    build_artifact_audit, build_determinism_audit, build_release_health, governance_status, memory,
    promotion_ready_approved,
};

pub fn build_preflight_gate(
    project_root: &str,
    memory_root: &str,
) -> Result<PreflightGateReport, String> {
    let artifact = build_artifact_audit(project_root)?;
    let determinism = build_determinism_audit(project_root, memory_root)?;
    let health = build_release_health(project_root, memory_root)?;
    let governance = governance_status(project_root, memory_root)?;
    let ready_approved = promotion_ready_approved(project_root, memory_root)?;
    let mut blockers = Vec::new();
    let mut warnings = Vec::new();

    if health.auto_promote || governance.auto_promote {
        blockers.push("auto_promote_true".to_string());
    }
    if !artifact.sandbox_leaks.is_empty() {
        blockers.push("sandbox_leaks".to_string());
    }
    if !artifact.tracked_runtime_artifacts.is_empty() {
        blockers.push("tracked_runtime_artifacts".to_string());
    }
    if !determinism.deterministic_enough {
        blockers.push("determinism_audit_failed".to_string());
    }
    if ready_approved.is_empty() {
        warnings.push("no_approved_release_candidate".to_string());
    }
    if health.health_grade == "red" {
        blockers.push("release_health_red".to_string());
    }

    blockers.sort();
    blockers.dedup();
    warnings.sort();
    warnings.dedup();

    let gate_status = if !blockers.is_empty() {
        "fail"
    } else if !warnings.is_empty() || ready_approved.is_empty() {
        "warn"
    } else {
        "pass"
    }
    .to_string();
    let next_actions_ru = match gate_status.as_str() {
        "pass" => {
            vec!["Можно создавать metadata-only release bundle для approved кандидата.".to_string()]
        }
        "warn" => vec![
            "Система безопасна, но нет approved release-кандидата.".to_string(),
            "Запустите promotion queue, replay/review и operator approval для кандидата."
                .to_string(),
        ],
        _ => vec![
            "Устраните blockers перед release.".to_string(),
            "Не запускайте promotion; auto_promote должен оставаться false.".to_string(),
        ],
    };

    Ok(PreflightGateReport {
        generated_at: memory::now_unix(),
        gate_status,
        release_preflight_status: if ready_approved.is_empty() {
            "no_candidate".to_string()
        } else {
            "candidate_available".to_string()
        },
        governance_status: if governance.operator_approval_required && !governance.auto_promote {
            "ready".to_string()
        } else {
            "blocked".to_string()
        },
        artifact_audit_status: if artifact.should_fail_release {
            "fail".to_string()
        } else {
            "pass".to_string()
        },
        determinism_status: if determinism.deterministic_enough {
            "pass".to_string()
        } else {
            "fail".to_string()
        },
        health_grade: health.health_grade,
        auto_promote: false,
        operator_approval_required: true,
        blockers,
        warnings,
        next_actions_ru,
    })
}

pub fn print_preflight_gate(project_root: &str, memory_root: &str) -> Result<String, String> {
    let gate = build_preflight_gate(project_root, memory_root)?;
    Ok(format!(
        "preflight_gate: status={} health={} auto_promote={} approval_required={} blockers={} warnings={}\nnext_actions={}",
        gate.gate_status,
        gate.health_grade,
        gate.auto_promote,
        gate.operator_approval_required,
        if gate.blockers.is_empty() { "none".to_string() } else { gate.blockers.join(",") },
        if gate.warnings.is_empty() { "none".to_string() } else { gate.warnings.join(",") },
        gate.next_actions_ru.join("; ")
    ))
}

pub fn print_preflight_gate_json(project_root: &str, memory_root: &str) -> Result<String, String> {
    serde_json::to_string_pretty(&build_preflight_gate(project_root, memory_root)?)
        .map_err(|error| format!("failed to serialize preflight gate: {error}"))
}
