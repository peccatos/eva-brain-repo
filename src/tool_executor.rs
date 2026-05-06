use std::fs;
use std::path::Path;
use std::process::Command;

use crate::tool_contract::{CommandOutput, ToolRequest, ToolResponse};

#[derive(Debug, Clone)]
pub struct ToolExecutor {
    output_limit: usize,
}

impl Default for ToolExecutor {
    fn default() -> Self {
        Self {
            output_limit: 16_384,
        }
    }
}

impl ToolExecutor {
    pub fn new(output_limit: usize) -> Self {
        Self { output_limit }
    }

    pub fn run(&self, request: ToolRequest) -> Result<ToolResponse, String> {
        match request {
            ToolRequest::WriteFile { path, contents } => {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).map_err(|error| {
                        format!("failed to create {}: {}", parent.display(), error)
                    })?;
                }
                fs::write(&path, contents.as_bytes())
                    .map_err(|error| format!("failed to write {}: {}", path.display(), error))?;
                Ok(ToolResponse::Write {
                    bytes_written: contents.len() as u64,
                })
            }
            ToolRequest::RemoveFile { path } => {
                let existed = path.exists();
                if existed {
                    fs::remove_file(&path).map_err(|error| {
                        format!("failed to remove {}: {}", path.display(), error)
                    })?;
                }
                Ok(ToolResponse::Remove { existed })
            }
            ToolRequest::CargoCheck { workdir } => Ok(ToolResponse::Command(run_cargo(
                Path::new(&workdir),
                &["check"],
                self.output_limit,
            )?)),
            ToolRequest::CargoTest { workdir, args } => {
                let mut command_args = vec!["test".to_string()];
                command_args.extend(args);
                let refs = command_args.iter().map(String::as_str).collect::<Vec<_>>();
                Ok(ToolResponse::Command(run_cargo(
                    Path::new(&workdir),
                    &refs,
                    self.output_limit,
                )?))
            }
        }
    }
}

fn run_cargo(workdir: &Path, args: &[&str], output_limit: usize) -> Result<CommandOutput, String> {
    let output = Command::new("cargo")
        .args(args)
        .current_dir(workdir)
        .env("CARGO_TERM_COLOR", "never")
        .output()
        .map_err(|error| format!("failed to spawn cargo in {}: {}", workdir.display(), error))?;

    Ok(CommandOutput {
        success: output.status.success(),
        exit_code: output.status.code(),
        stdout: truncate_utf8(&output.stdout, output_limit),
        stderr: truncate_utf8(&output.stderr, output_limit),
        truncated: output.stdout.len() > output_limit || output.stderr.len() > output_limit,
    })
}

fn truncate_utf8(bytes: &[u8], limit: usize) -> String {
    let take = bytes.len().min(limit);
    String::from_utf8_lossy(&bytes[..take]).to_string()
}
