use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ReleaseLedgerRecord {
    #[serde(default)]
    pub release_id: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub gate_status: String,
    #[serde(default)]
    pub health_grade: String,
    #[serde(default)]
    pub approved_candidate_count: usize,
    #[serde(default)]
    pub generated_at: u64,
}
