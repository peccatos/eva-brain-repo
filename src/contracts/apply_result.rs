use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ApplyStatus {
    Applied,
    Refused,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyResult {
    pub apply_id: String,
    pub proposal_id: String,
    pub task_id: String,
    pub status: ApplyStatus,
    pub applied_at: u64,
    pub files_changed: Vec<String>,
    pub snapshot_id: Option<String>,
    pub rollback_manifest: Option<String>,
    pub warnings: Vec<String>,
    pub blockers: Vec<String>,
}
