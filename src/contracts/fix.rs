use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FixMode {
    DryRun,
    Apply,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FixOnly {
    CargoCheck,
    Ci,
    Tests,
    Docs,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FixRiskCap {
    Low,
    Medium,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FixStatus {
    Detected,
    ProposalCreated,
    DryRunPassed,
    DryRunFailed,
    Applied,
    ValidationPassed,
    ValidationFailed,
    NoActionableProblem,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FixProblemKind {
    CargoCheckFailure,
    MissingCi,
    MissingSmokeTest,
    MissingReadmeValidation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixRequest {
    pub fix_id: String,
    pub target_path: PathBuf,
    pub dry_run: bool,
    pub apply: bool,
    pub only: Option<FixOnly>,
    pub max_files: usize,
    pub risk_cap: FixRiskCap,
    pub no_llm: bool,
    pub provider: Option<String>,
    pub evidence_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixDetectedProblem {
    pub kind: FixProblemKind,
    pub description: String,
    pub goal: String,
    pub project_type: String,
    pub files_planned: Vec<String>,
    pub validation_commands: Vec<String>,
    pub workspace_dirty: bool,
    pub workspace_changes: Vec<String>,
    pub details: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixEvidencePaths {
    pub evidence_dir: PathBuf,
    pub request_json: PathBuf,
    pub detection_json: PathBuf,
    pub proposal_json: PathBuf,
    pub dry_run_json: PathBuf,
    pub apply_result_json: Option<PathBuf>,
    pub validation_json: Option<PathBuf>,
    pub report_md: PathBuf,
    pub report_json: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixReport {
    pub fix_id: String,
    pub target_path: PathBuf,
    pub mode: FixMode,
    pub project_type: String,
    pub workspace_dirty: bool,
    pub detected_problem: Option<String>,
    pub risk: String,
    pub files_planned: Vec<String>,
    pub files_changed: Vec<String>,
    pub validation_commands: Vec<String>,
    pub status: FixStatus,
    pub evidence_dir: PathBuf,
    pub warnings: Vec<String>,
    pub blockers: Vec<String>,
    pub provider: String,
    pub llm_used: bool,
}
