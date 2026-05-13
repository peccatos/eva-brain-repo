use std::process::Command;

use crate::agent::storage::{id, memory_path, now_unix, save_json_pretty};
use crate::contracts::{AgentValidationStatus, ValidationCommandResult, ValidationRun};

pub fn run_validation(project_root: &str, memory_root: &str) -> Result<ValidationRun, String> {
    let started = now_unix();
    let commands = vec![
        run_cargo(project_root, &["fmt", "--check"]),
        run_cargo(project_root, &["check"]),
        run_cargo(project_root, &["test"]),
    ];
    let status = if commands.iter().all(|command| command.success) {
        AgentValidationStatus::Passed
    } else if commands.iter().any(|command| command.success) {
        AgentValidationStatus::Partial
    } else {
        AgentValidationStatus::Failed
    };
    let run = ValidationRun {
        validation_id: id("validation"),
        task_id: None,
        proposal_id: None,
        status,
        started_at: started,
        finished_at: now_unix(),
        commands,
        warnings: Vec::new(),
        blockers: Vec::new(),
    };
    save_json_pretty(
        &memory_path(
            memory_root,
            &["validations", &format!("{}.json", run.validation_id)],
        ),
        &run,
    )?;
    save_json_pretty(
        &memory_path(memory_root, &["validations", "latest_validation.json"]),
        &run,
    )?;
    Ok(run)
}

pub fn run_cargo(project_root: &str, args: &[&str]) -> ValidationCommandResult {
    let output = Command::new("cargo")
        .args(args)
        .current_dir(project_root)
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("CARGO_BUILD_TARGET_DIR")
        .output();
    match output {
        Ok(output) => ValidationCommandResult {
            command: format!("cargo {}", args.join(" ")),
            exit_code: output.status.code(),
            success: output.status.success(),
            stdout_tail: tail(&String::from_utf8_lossy(&output.stdout), 4000),
            stderr_tail: tail(&String::from_utf8_lossy(&output.stderr), 4000),
        },
        Err(error) => ValidationCommandResult {
            command: format!("cargo {}", args.join(" ")),
            exit_code: None,
            success: false,
            stdout_tail: String::new(),
            stderr_tail: error.to_string(),
        },
    }
}

pub fn print_validation_run(project_root: &str, memory_root: &str) -> Result<String, String> {
    let run = run_validation(project_root, memory_root)?;
    Ok(format!(
        "EVA Validation Run\nstatus={}\ncommands={}",
        match run.status {
            AgentValidationStatus::Passed => "passed",
            AgentValidationStatus::Failed => "failed",
            AgentValidationStatus::Partial => "partial",
            AgentValidationStatus::NotRun => "not_run",
        },
        run.commands
            .iter()
            .map(|cmd| cmd.command.as_str())
            .collect::<Vec<_>>()
            .join(",")
    ))
}

fn tail(value: &str, max: usize) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    let start = chars.len().saturating_sub(max);
    chars[start..].iter().collect()
}
