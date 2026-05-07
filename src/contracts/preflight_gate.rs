use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PreflightGateReport {
    #[serde(default)]
    pub generated_at: u64,
    #[serde(default)]
    pub gate_status: String,
    #[serde(default)]
    pub release_preflight_status: String,
    #[serde(default)]
    pub governance_status: String,
    #[serde(default)]
    pub artifact_audit_status: String,
    #[serde(default)]
    pub determinism_status: String,
    #[serde(default)]
    pub health_grade: String,
    #[serde(default)]
    pub auto_promote: bool,
    #[serde(default)]
    pub operator_approval_required: bool,
    #[serde(default)]
    pub blockers: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub next_actions_ru: Vec<String>,
}
