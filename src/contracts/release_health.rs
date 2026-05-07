use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ReleaseHealthReport {
    #[serde(default)]
    pub generated_at: u64,
    #[serde(default)]
    pub release_runtime_support: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_release_id: Option<String>,
    #[serde(default)]
    pub release_count: usize,
    #[serde(default)]
    pub candidate_count: usize,
    #[serde(default)]
    pub approved_count: usize,
    #[serde(default)]
    pub ready_count: usize,
    #[serde(default)]
    pub blocked_count: usize,
    #[serde(default)]
    pub replay_passed_candidates: usize,
    #[serde(default)]
    pub promoted_candidates: usize,
    #[serde(default)]
    pub governance_ready: bool,
    #[serde(default)]
    pub proof_ready: bool,
    #[serde(default)]
    pub preflight_ready: bool,
    #[serde(default)]
    pub sandbox_leaks_detected: bool,
    #[serde(default)]
    pub auto_promote: bool,
    #[serde(default)]
    pub operator_approval_required: bool,
    #[serde(default)]
    pub health_score: u32,
    #[serde(default)]
    pub health_grade: String,
    #[serde(default)]
    pub blockers: Vec<String>,
    #[serde(default)]
    pub recommendations_ru: Vec<String>,
}
