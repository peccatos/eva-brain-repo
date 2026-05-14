use crate::ValidationCommandResult;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DoctorFindingLevel {
    Ok,
    Info,
    Warn,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DoctorProjectType {
    Rust,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DoctorStatus {
    Passed,
    Warn,
    Critical,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorRequest {
    pub doctor_id: String,
    pub target_path: PathBuf,
    pub validate: bool,
    pub json: bool,
    pub no_llm: bool,
    pub evidence_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorFinding {
    pub level: DoctorFindingLevel,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorSuggestion {
    pub command: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorValidationSummary {
    pub status: String,
    pub commands: Vec<ValidationCommandResult>,
    pub git_status_before: Vec<String>,
    pub git_status_after: Vec<String>,
    pub validation_side_effects: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorEvidencePaths {
    pub evidence_dir: PathBuf,
    pub request_json: PathBuf,
    pub report_json: PathBuf,
    pub report_md: PathBuf,
    pub validation_json: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub doctor_id: String,
    pub target_path: PathBuf,
    pub project_type: DoctorProjectType,
    pub status: DoctorStatus,
    pub health_score: u8,
    pub workspace_dirty: bool,
    pub source_mutation: bool,
    pub evidence_written: bool,
    pub findings: Vec<DoctorFinding>,
    pub suggestions: Vec<DoctorSuggestion>,
    pub validation: Option<DoctorValidationSummary>,
    pub evidence_dir: Option<PathBuf>,
    pub warnings: Vec<String>,
    pub blockers: Vec<String>,
}
