use crate::evolution::{
    build_preflight_gate, build_release_health, latest_release_or_none, print_future_phases,
};

pub fn print_operator_runbook(project_root: &str, memory_root: &str) -> Result<String, String> {
    let health = build_release_health(project_root, memory_root)?;
    let gate = build_preflight_gate(project_root, memory_root)?;
    let latest_release = latest_release_or_none(memory_root)?;
    let release_safe = gate.gate_status == "pass";
    let next_command = if release_safe {
        "cargo run -- --release-status"
    } else if gate.gate_status == "warn" {
        "cargo run -- --promotion-queue"
    } else {
        "cargo run -- --artifact-audit"
    };
    Ok(format!(
        "# EVA Operator Runbook\n\nСтатус: release_health={} score={} preflight_gate={} latest_release={}\nБезопасность release: {}\nСледующая команда: {}\nBlockers: {}\nFuture phases allowed_now=false:\n{}\n\nНапоминание: auto_promote=false, operator approval обязателен.",
        health.health_grade,
        health.health_score,
        gate.gate_status,
        latest_release,
        if release_safe { "да, metadata-only" } else { "нет или требуется операторская подготовка" },
        next_command,
        if gate.blockers.is_empty() { "none".to_string() } else { gate.blockers.join(", ") },
        print_future_phases()
    ))
}
