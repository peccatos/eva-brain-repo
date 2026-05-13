use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RuntimeValidation {
    #[serde(default)]
    pub validation_id: String,
    #[serde(default)]
    pub generated_at: u64,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub blockers: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub checks: Vec<String>,
    #[serde(default)]
    pub next_actions: Vec<String>,
    #[serde(default)]
    pub green_conditions: Vec<String>,
    #[serde(default)]
    pub missing_green_conditions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_release_candidate: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_bundle: Option<String>,
    #[serde(default)]
    pub metrics_summary: String,
    #[serde(default)]
    pub candidate_queue_summary: String,
    #[serde(default)]
    pub sandbox_state: String,
    #[serde(default)]
    pub auto_promote: bool,
    #[serde(default)]
    pub operator_approval_required: bool,
}
