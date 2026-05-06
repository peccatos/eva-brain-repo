use std::process::Command;
use std::time::Instant;

use crate::contracts::sandbox_result::CommandResult;
use crate::sandbox::limits::{MAX_STDERR_BYTES, MAX_STDOUT_BYTES};

pub fn run_cargo_check(path: &str) -> CommandResult {
    run_command(path, "cargo", &["check"])
}

pub fn run_cargo_test(path: &str) -> CommandResult {
    run_command(path, "cargo", &["test"])
}

pub fn run_cargo_run(path: &str) -> CommandResult {
    run_command(path, "cargo", &["run", "--", "--once"])
}

fn run_command(path: &str, bin: &str, args: &[&str]) -> CommandResult {
    let start = Instant::now();
    let output = Command::new(bin).args(args).current_dir(path).output();
    let duration_ms = start.elapsed().as_millis();

    match output {
        Ok(out) => CommandResult {
            success: out.status.success(),
            stdout: trim_output(
                String::from_utf8_lossy(&out.stdout).to_string(),
                MAX_STDOUT_BYTES,
            ),
            stderr: trim_output(
                String::from_utf8_lossy(&out.stderr).to_string(),
                MAX_STDERR_BYTES,
            ),
            duration_ms,
        },
        Err(error) => CommandResult {
            success: false,
            stdout: String::new(),
            stderr: format!("failed to run command: {error}"),
            duration_ms,
        },
    }
}

fn trim_output(mut value: String, limit: usize) -> String {
    if value.len() > limit {
        value.truncate(limit);
        value.push_str("\n[output truncated]");
    }
    value
}
