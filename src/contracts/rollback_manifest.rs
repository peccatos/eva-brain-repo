use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RollbackManifest {
    #[serde(default)]
    pub release_id: String,
    #[serde(default)]
    pub source_run_id: String,
    #[serde(default)]
    pub target_file: String,
    #[serde(default)]
    pub rollback_type: String,
    #[serde(default)]
    pub rollback_available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_candidate_report_path: Option<String>,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub created_at: u64,
}
