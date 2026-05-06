use serde::{Deserialize, Serialize};

pub use crate::contracts::validation::ValidationStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvolutionStatus {
    Passed,
    Failed,
    Candidate,
    Promoted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvolutionLogEntry {
    pub run_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hypothesis_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objective: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub graph_evidence: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recombined_source_patterns: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recombined_avoided_risks: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recombination_reason_ru: Option<String>,
    pub mutation_id: String,
    #[serde(default)]
    pub mutation_digest: String,
    pub status: EvolutionStatus,
    pub target_file: String,
    pub mutation_kind: String,
    pub risk: f32,
    pub score: f32,
    #[serde(default)]
    pub useful_change: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub non_candidate_reason: Option<String>,
    #[serde(default)]
    pub duplicate_rejected: bool,
    #[serde(default)]
    pub regression_penalty: f32,
    #[serde(default)]
    pub success_bonus: f32,
    pub cargo_check_ok: bool,
    pub cargo_test_ok: bool,
    pub cargo_run_ok: bool,
    pub retained_in_core: bool,
    pub sandbox_destroyed: bool,
    pub stdout_digest: String,
    pub stderr_digest: String,
    pub stderr_tail: String,
    pub timestamp_unix: u64,
}
