use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TuiDashboardState {
    #[serde(default)]
    pub runtime_status: String,
    #[serde(default)]
    pub runtime_validation_status: String,
    #[serde(default)]
    pub autonomy_level: u8,
    #[serde(default)]
    pub allowed_next_autonomy_level: u8,
    #[serde(default)]
    pub campaign_mode_allowed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_run_id: Option<String>,
    #[serde(default)]
    pub last_replay_status: String,
    #[serde(default)]
    pub candidate_count: u64,
    #[serde(default)]
    pub ready_candidates: usize,
    #[serde(default)]
    pub blocked_candidates: usize,
    #[serde(default)]
    pub quarantined_candidates: usize,
    #[serde(default)]
    pub duplicate_candidates: usize,
    #[serde(default)]
    pub unreplayable_candidates: usize,
    #[serde(default)]
    pub release_status: String,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub missing_green_conditions: Vec<String>,
    #[serde(default)]
    pub blockers: Vec<String>,
    #[serde(default)]
    pub sandbox_leak_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TuiRunRow {
    #[serde(default)]
    pub run_id: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub replay_status: String,
    #[serde(default)]
    pub cargo_test_ok: Option<bool>,
    #[serde(default)]
    pub cargo_run_ok: Option<bool>,
    #[serde(default)]
    pub duplicate_rejected: bool,
    #[serde(default)]
    pub candidate: bool,
    #[serde(default)]
    pub promoted: bool,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TuiCandidateRow {
    #[serde(default)]
    pub run_id: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub mutation_kind: String,
    #[serde(default)]
    pub mutation_class: String,
    #[serde(default)]
    pub target_file: String,
    #[serde(default)]
    pub score: f32,
    #[serde(default)]
    pub risk: f32,
    #[serde(default)]
    pub promotion_eligibility: String,
    #[serde(default)]
    pub promotion_allowed: bool,
    #[serde(default)]
    pub replay_status: String,
    #[serde(default)]
    pub block_reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cargo_test_ok: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cargo_run_ok: Option<bool>,
    #[serde(default)]
    pub duplicate_rejected: bool,
    #[serde(default)]
    pub promoted: bool,
    #[serde(default)]
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TuiMetricsState {
    #[serde(default)]
    pub total_runs: u64,
    #[serde(default)]
    pub passed_runs: u64,
    #[serde(default)]
    pub failed_runs: u64,
    #[serde(default)]
    pub safety_rejected_runs: u64,
    #[serde(default)]
    pub duplicate_rejected_runs: u64,
    #[serde(default)]
    pub replay_passed: u64,
    #[serde(default)]
    pub replay_failed: u64,
    #[serde(default)]
    pub candidate_count: u64,
    #[serde(default)]
    pub promoted_count: u64,
    #[serde(default)]
    pub average_score: f32,
    #[serde(default)]
    pub pass_ratio: f32,
    #[serde(default)]
    pub replay_pass_ratio: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TuiReleaseState {
    #[serde(default)]
    pub approved_release_candidate_exists: bool,
    #[serde(default)]
    pub release_bundle_exists: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_release_candidate: Option<String>,
    #[serde(default)]
    pub operator_approval_state: String,
    #[serde(default)]
    pub preflight_gate_status: String,
    #[serde(default)]
    pub release_health: String,
    #[serde(default)]
    pub green_gate_readiness: String,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub missing_green_conditions: Vec<String>,
    #[serde(default)]
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TuiAgentState {
    #[serde(default)]
    pub latest_task_id: String,
    #[serde(default)]
    pub task_count: usize,
    #[serde(default)]
    pub latest_task_goal: String,
    #[serde(default)]
    pub latest_task_status: String,
    #[serde(default)]
    pub latest_plan_id: String,
    #[serde(default)]
    pub latest_proposal_id: String,
    #[serde(default)]
    pub latest_validation_id: String,
    #[serde(default)]
    pub latest_validation_status: String,
    #[serde(default)]
    pub latest_report_id: String,
    #[serde(default)]
    pub latest_pr_summary_id: String,
    #[serde(default)]
    pub llm_provider: String,
    #[serde(default)]
    pub openai_configured: bool,
    #[serde(default)]
    pub llm_model: String,
    #[serde(default)]
    pub fallback_available: bool,
    #[serde(default)]
    pub production_agent_v1_ready: bool,
    #[serde(default)]
    pub readiness_blockers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TuiState {
    #[serde(default)]
    pub dashboard: TuiDashboardState,
    #[serde(default)]
    pub runs: Vec<TuiRunRow>,
    #[serde(default)]
    pub candidates: Vec<TuiCandidateRow>,
    #[serde(default)]
    pub metrics: TuiMetricsState,
    #[serde(default)]
    pub release: TuiReleaseState,
    #[serde(default)]
    pub agent: TuiAgentState,
    #[serde(default)]
    pub logs: Vec<String>,
    #[serde(default)]
    pub parse_messages: Vec<String>,
}
