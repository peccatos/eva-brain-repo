use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProofReport {
    #[serde(default)]
    pub generated_at: u64,
    #[serde(default)]
    pub local_corpus_ingestion_support: bool,
    #[serde(default)]
    pub read_only_corpus_safety: bool,
    #[serde(default)]
    pub task_suggestion_support: bool,
    #[serde(default)]
    pub campaign_diagnostics_support: bool,
    #[serde(default)]
    pub zero_yield_task_adjustment_support: bool,
    #[serde(default)]
    pub bounded_campaign_loop_support: bool,
    #[serde(default)]
    pub recombination_fallback_support: bool,
    #[serde(default)]
    pub replay_review_support: bool,
    #[serde(default)]
    pub promotion_queue_support: bool,
    #[serde(default)]
    pub supervised_task_support: bool,
    #[serde(default)]
    pub governance_runtime_support: bool,
    #[serde(default)]
    pub release_runtime_support: bool,
    #[serde(default)]
    pub release_health_support: bool,
    #[serde(default)]
    pub artifact_audit_support: bool,
    #[serde(default)]
    pub determinism_audit_support: bool,
    #[serde(default)]
    pub preflight_gate_v2_support: bool,
    #[serde(default)]
    pub release_ledger_support: bool,
    #[serde(default)]
    pub future_phase_registry_support: bool,
    #[serde(default)]
    pub operator_runbook_support: bool,
    #[serde(default)]
    pub auto_promote: bool,
    #[serde(default)]
    pub operator_approval_required: bool,
    #[serde(default)]
    pub forbidden_target_preservation: bool,
    #[serde(default)]
    pub total_runs: u64,
    #[serde(default)]
    pub candidate_count: usize,
    #[serde(default)]
    pub replay_passed_candidates: usize,
    #[serde(default)]
    pub promoted_candidates: usize,
    #[serde(default)]
    pub ready_candidates: usize,
    #[serde(default)]
    pub blocked_candidates: usize,
    #[serde(default)]
    pub approved_count: usize,
    #[serde(default)]
    pub rejected_count: usize,
    #[serde(default)]
    pub deferred_count: usize,
    #[serde(default)]
    pub release_count: usize,
    #[serde(default)]
    pub release_ledger_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_release_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_bounded_run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_supervised_run_id: Option<String>,
}
