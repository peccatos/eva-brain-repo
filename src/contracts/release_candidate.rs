use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ReleaseCandidateState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_release_candidate: Option<String>,
    #[serde(default)]
    pub approved_count: u64,
    #[serde(default)]
    pub release_bundle_exists: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_release_bundle: Option<String>,
    #[serde(default)]
    pub operator_approval_required: bool,
    #[serde(default)]
    pub operator_approved: bool,
    #[serde(default)]
    pub preflight_gate_v3: String,
    #[serde(default)]
    pub release_health: String,
    #[serde(default)]
    pub blockers: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ReleaseCandidateApprovalReport {
    #[serde(default)]
    pub run_id: String,
    #[serde(default)]
    pub candidate_state: String,
    #[serde(default)]
    pub replay_status: String,
    #[serde(default)]
    pub cargo_test_ok: Option<bool>,
    #[serde(default)]
    pub cargo_run_ok: Option<bool>,
    #[serde(default)]
    pub operator_approved: bool,
    #[serde(default)]
    pub evidence_bundle_path: String,
    #[serde(default)]
    pub validation_report_path: String,
    #[serde(default)]
    pub release_candidate_path: String,
    #[serde(default)]
    pub blockers: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub generated_at: u64,
}
