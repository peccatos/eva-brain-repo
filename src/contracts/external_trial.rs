use crate::{DoctorFinding, FixOnly, FixReport};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalTrialRequest {
    pub trial_id: String,
    pub repo_paths: Vec<PathBuf>,
    pub output_dir: PathBuf,
    pub json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalTrialRepoResult {
    pub repo_path: PathBuf,
    pub repo_name: String,
    pub exists: bool,
    pub is_directory: bool,
    pub doctor_status: String,
    pub doctor_health_score: u8,
    pub doctor_findings: Vec<DoctorFinding>,
    pub suggested_fix_commands: Vec<String>,
    pub dry_run_fix_reports: Vec<FixReport>,
    pub source_mutation: bool,
    pub evidence_dir: PathBuf,
    pub status: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalTrialReport {
    pub trial_id: String,
    pub repos_total: usize,
    pub repos_processed: usize,
    pub repos_skipped: usize,
    pub repo_results: Vec<ExternalTrialRepoResult>,
    pub warnings: Vec<String>,
    pub blockers: Vec<String>,
    pub output_dir: PathBuf,
}

pub fn parse_fix_only_from_command(command: &str) -> Option<FixOnly> {
    if command.contains("--only cargo-check") {
        Some(FixOnly::CargoCheck)
    } else if command.contains("--only ci") {
        Some(FixOnly::Ci)
    } else if command.contains("--only tests") {
        Some(FixOnly::Tests)
    } else if command.contains("--only docs") {
        Some(FixOnly::Docs)
    } else if command.contains("--only hygiene") {
        Some(FixOnly::Hygiene)
    } else {
        None
    }
}
