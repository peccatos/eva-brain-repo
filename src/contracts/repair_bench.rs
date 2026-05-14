use crate::FixOnly;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RepairBenchStatus {
    Passed,
    Warn,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RepairBenchCaseStatus {
    Passed,
    Failed,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairBenchRequest {
    pub bench_id: String,
    pub suite: String,
    pub output_dir: PathBuf,
    pub no_llm: bool,
    pub json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairBenchCase {
    pub case_id: String,
    pub kind: String,
    pub target_path: PathBuf,
    pub expected_problem: Option<String>,
    pub fix_only: Option<FixOnly>,
    pub apply: bool,
    pub expected_files: Vec<String>,
    pub validation_expected: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairBenchCaseResult {
    pub case_id: String,
    pub kind: String,
    pub target_path: PathBuf,
    pub detected_problem: Option<String>,
    pub expected_problem: Option<String>,
    pub fix_status: String,
    pub validation_passed: bool,
    pub evidence_dir: Option<PathBuf>,
    pub files_expected: Vec<String>,
    pub files_observed: Vec<String>,
    pub status: RepairBenchCaseStatus,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairBenchCaseHistoryStatus {
    pub case_id: String,
    pub status: RepairBenchCaseStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairBenchEvidencePaths {
    pub output_dir: PathBuf,
    pub request_json: PathBuf,
    pub report_json: PathBuf,
    pub report_md: PathBuf,
    pub cases_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairBenchMetricSummary {
    pub total_cases: usize,
    pub actionable_cases: usize,
    pub passed_cases: usize,
    pub failed_cases: usize,
    pub partial_cases: usize,
    pub detection_success_rate: f64,
    pub repair_success_rate: f64,
    pub validation_success_rate: f64,
    pub evidence_success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairBenchReport {
    pub bench_id: String,
    pub suite: String,
    pub status: RepairBenchStatus,
    pub total_cases: usize,
    pub passed_cases: usize,
    pub failed_cases: usize,
    pub partial_cases: usize,
    pub case_results: Vec<RepairBenchCaseResult>,
    pub metrics: RepairBenchMetricSummary,
    pub output_dir: PathBuf,
    pub warnings: Vec<String>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairBenchHistoryEntry {
    pub bench_id: String,
    pub suite: String,
    pub status: RepairBenchStatus,
    pub total_cases: usize,
    pub passed_cases: usize,
    pub partial_cases: usize,
    pub failed_cases: usize,
    pub detection_success_rate: f64,
    pub repair_success_rate: f64,
    pub validation_success_rate: f64,
    pub evidence_success_rate: f64,
    pub case_statuses: Vec<RepairBenchCaseHistoryStatus>,
    pub generated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairBenchHistoryReport {
    pub output_dir: PathBuf,
    pub history_path: PathBuf,
    pub latest_path: PathBuf,
    pub runs: usize,
    pub entries: Vec<RepairBenchHistoryEntry>,
    pub latest: Option<RepairBenchHistoryEntry>,
    pub warnings: Vec<String>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RepairBenchGateStatus {
    Passed,
    Warn,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairBenchBaseline {
    pub suite: String,
    pub total_cases: usize,
    pub actionable_cases: usize,
    pub passed_cases: usize,
    pub partial_cases: usize,
    pub failed_cases: usize,
    pub detection_success_rate: f64,
    pub repair_success_rate: f64,
    pub validation_success_rate: f64,
    pub evidence_success_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairBenchRegression {
    pub field: String,
    pub baseline: String,
    pub current: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairBenchGateRequest {
    pub suite: String,
    pub baseline: String,
    pub baseline_file: Option<PathBuf>,
    pub output_dir: PathBuf,
    pub json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairBenchGateReport {
    pub bench_id: String,
    pub suite: String,
    pub status: RepairBenchGateStatus,
    pub baseline: RepairBenchBaseline,
    pub current: RepairBenchBaseline,
    pub current_report: RepairBenchReport,
    pub regressions: Vec<RepairBenchRegression>,
    pub output_dir: PathBuf,
    pub warnings: Vec<String>,
    pub blockers: Vec<String>,
}
