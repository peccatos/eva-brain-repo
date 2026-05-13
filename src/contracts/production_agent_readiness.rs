use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProductionAgentReadiness {
    pub task_intake_ok: bool,
    pub workspace_inspection_ok: bool,
    pub planning_ok: bool,
    pub llm_adapter_ok: bool,
    pub rule_based_fallback_ok: bool,
    pub proposal_ok: bool,
    pub approval_gate_ok: bool,
    pub safe_apply_ok: bool,
    pub validation_ok: bool,
    pub report_ok: bool,
    pub pr_summary_ok: bool,
    pub tui_agent_visibility_ok: bool,
    pub safety_policy_ok: bool,
    pub production_agent_v1_ready: bool,
    pub warnings: Vec<String>,
    pub blockers: Vec<String>,
}
