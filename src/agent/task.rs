use std::fs;
use std::path::Path;

use crate::agent::storage::{id, load_json, memory_path, now_unix, save_json_pretty};
use crate::contracts::{AgentTask, AgentTaskStatus};

pub fn default_scope() -> Vec<String> {
    vec![
        "src/".into(),
        "tests/".into(),
        "docs/".into(),
        "README.md".into(),
    ]
}

pub fn default_forbidden_paths() -> Vec<String> {
    vec![
        ".git/".into(),
        "target/".into(),
        "memory/".into(),
        "releases/".into(),
        "sandboxes/".into(),
        ".eva-runtime-tests/".into(),
        ".eva-evolution-tests/".into(),
    ]
}

pub fn create_task(memory_root: &str, goal: &str) -> Result<AgentTask, String> {
    let now = now_unix();
    let task = AgentTask {
        task_id: id("task"),
        goal: goal.to_string(),
        status: AgentTaskStatus::Created,
        scope: default_scope(),
        forbidden_paths: default_forbidden_paths(),
        risk_level: "low".into(),
        approval_required: true,
        created_at: now,
        updated_at: now,
        inspection_id: None,
        plan_id: None,
        proposal_id: None,
        approval_id: None,
        apply_id: None,
        validation_id: None,
        report_id: None,
        pr_summary_id: None,
        warnings: Vec::new(),
        blockers: Vec::new(),
    };
    save_task(memory_root, &task)?;
    save_json_pretty(
        &memory_path(memory_root, &["tasks", "latest_task.json"]),
        &task,
    )?;
    Ok(task)
}

pub fn save_task(memory_root: &str, task: &AgentTask) -> Result<(), String> {
    save_json_pretty(&task_path(memory_root, &task.task_id), task)
}

pub fn load_task(memory_root: &str, task_id: &str) -> Result<AgentTask, String> {
    load_json(&task_path(memory_root, task_id))
}

pub fn show_task(memory_root: &str, task_id: &str) -> Result<Option<AgentTask>, String> {
    let path = task_path(memory_root, task_id);
    if !path.exists() {
        return Ok(None);
    }
    load_json(&path).map(Some)
}

pub fn list_tasks(memory_root: &str) -> Result<Vec<AgentTask>, String> {
    let dir = memory_path(memory_root, &["tasks"]);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut tasks = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|error| format!("read tasks: {error}"))? {
        let path = entry
            .map_err(|error| format!("read task entry: {error}"))?
            .path();
        if path.file_name().and_then(|n| n.to_str()) == Some("latest_task.json") {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            if let Ok(task) = load_json::<AgentTask>(&path) {
                tasks.push(task);
            }
        }
    }
    tasks.sort_by(|a, b| a.task_id.cmp(&b.task_id));
    Ok(tasks)
}

pub fn update_task(memory_root: &str, mut task: AgentTask) -> Result<AgentTask, String> {
    task.updated_at = now_unix();
    save_task(memory_root, &task)?;
    save_json_pretty(
        &memory_path(memory_root, &["tasks", "latest_task.json"]),
        &task,
    )?;
    Ok(task)
}

pub fn task_path(memory_root: &str, task_id: &str) -> std::path::PathBuf {
    Path::new(memory_root)
        .join("tasks")
        .join(format!("{task_id}.json"))
}

pub fn print_create_task(memory_root: &str, goal: &str) -> Result<String, String> {
    let task = create_task(memory_root, goal)?;
    Ok(format!(
        "task created\ntask_id={}\ngoal={}\nstatus=Created\napproval_required=true",
        task.task_id, task.goal
    ))
}

pub fn print_tasks(memory_root: &str) -> Result<String, String> {
    let tasks = list_tasks(memory_root)?;
    let latest = tasks.last().map(|t| t.task_id.as_str()).unwrap_or("none");
    Ok(format!(
        "EVA Agent Tasks\ncount={}\nlatest={latest}",
        tasks.len()
    ))
}

pub fn print_show_task(memory_root: &str, task_id: &str) -> Result<String, String> {
    let Some(task) = show_task(memory_root, task_id)? else {
        return Ok(format!("task not found\ntask_id={task_id}"));
    };
    Ok(format!(
        "EVA Agent Task\ntask_id={}\ngoal={}\nstatus={:?}",
        task.task_id, task.goal, task.status
    ))
}
