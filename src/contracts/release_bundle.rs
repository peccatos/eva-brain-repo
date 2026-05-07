use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ReleasePreflightReport {
    #[serde(default)]
    pub run_id: String,
    #[serde(default)]
    pub allowed: bool,
    #[serde(default)]
    pub reason_ru: String,
    #[serde(default)]
    pub blockers: Vec<String>,
    #[serde(default)]
    pub mutation_kind: String,
    #[serde(default)]
    pub mutation_class: String,
    #[serde(default)]
    pub target_file: String,
    #[serde(default)]
    pub replay_status: String,
    #[serde(default)]
    pub approval_required: bool,
    #[serde(default)]
    pub approved: bool,
    #[serde(default)]
    pub promotion_queue_state: String,
    #[serde(default)]
    pub risk: f32,
    #[serde(default)]
    pub score: f32,
    #[serde(default)]
    pub generated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ReleaseBundle {
    #[serde(default)]
    pub release_id: String,
    #[serde(default)]
    pub source_run_id: String,
    #[serde(default)]
    pub target_file: String,
    #[serde(default)]
    pub mutation_kind: String,
    #[serde(default)]
    pub mutation_class: String,
    #[serde(default)]
    pub score: f32,
    #[serde(default)]
    pub risk: f32,
    #[serde(default)]
    pub replay_status: String,
    #[serde(default)]
    pub approval_status: String,
    #[serde(default)]
    pub promotion_queue_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_report_path: Option<String>,
    #[serde(default)]
    pub preflight_report_path: String,
    #[serde(default)]
    pub release_manifest_path: String,
    #[serde(default)]
    pub rollback_manifest_path: String,
    #[serde(default)]
    pub changelog_path: String,
    #[serde(default)]
    pub candidate_diff_summary: String,
    #[serde(default)]
    pub safety_notes: Vec<String>,
    #[serde(default)]
    pub created_at: u64,
}
