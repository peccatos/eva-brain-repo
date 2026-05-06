use std::path::Path;

use crate::contracts::{DeniedMutationKind, TaskContract};

const HARD_FORBIDDEN_TARGETS: [&str; 4] = ["src/core/*", "src/main.rs", "src/lib.rs", "Cargo.toml"];
const REQUIRED_DENIED_KINDS: [DeniedMutationKind; 4] = [
    DeniedMutationKind::DeleteCode,
    DeniedMutationKind::RewriteFunction,
    DeniedMutationKind::FreeDiff,
    DeniedMutationKind::DependencyAdd,
];

pub fn validate_task_contract(task: &TaskContract) -> Result<(), String> {
    if task.cycles == 0 || task.cycles > 100 {
        return Err("cycles must be > 0 and <= 100".to_string());
    }
    if task.max_risk > 0.35 {
        return Err("max_risk must be <= 0.35".to_string());
    }
    if task.auto_promote {
        return Err("auto_promote must be false for now".to_string());
    }
    for target in HARD_FORBIDDEN_TARGETS {
        if !task.forbidden_targets.iter().any(|value| value == target) {
            return Err(format!("missing hard forbidden target: {target}"));
        }
    }
    for denied in REQUIRED_DENIED_KINDS {
        if !task.denied_mutation_kinds.contains(&denied) {
            return Err(format!("missing denied mutation kind: {:?}", denied));
        }
    }
    for pattern in task
        .allowed_targets
        .iter()
        .chain(task.forbidden_targets.iter())
    {
        validate_target_pattern(pattern)?;
    }
    Ok(())
}

pub fn load_task_contract(path: &Path) -> Result<TaskContract, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read task contract: {error}"))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("failed to parse task contract: {error}"))
}

pub fn store_task_contract(memory_root: &str, task: &TaskContract) -> Result<(), String> {
    crate::evolution::memory::write_json(
        Path::new(memory_root)
            .join("tasks")
            .join(format!("{}.task.json", task.task_id)),
        task,
    )
}

pub fn load_stored_task_contract(memory_root: &str, task_id: &str) -> Result<TaskContract, String> {
    load_task_contract(
        &Path::new(memory_root)
            .join("tasks")
            .join(format!("{}.task.json", task_id)),
    )
}

pub fn matches_target_patterns(target: &str, patterns: &[String]) -> bool {
    patterns
        .iter()
        .any(|pattern| matches_target_pattern(target, pattern))
}

fn matches_target_pattern(target: &str, pattern: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix('*') {
        target.starts_with(prefix)
    } else {
        target == pattern
    }
}

fn validate_target_pattern(pattern: &str) -> Result<(), String> {
    if pattern.contains("..") {
        return Err(format!(
            "path escape is forbidden in task pattern: {pattern}"
        ));
    }
    if Path::new(pattern).is_absolute() {
        return Err(format!("absolute task patterns are forbidden: {pattern}"));
    }
    Ok(())
}
