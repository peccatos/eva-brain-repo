use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentPlan {
    pub plan_id: String,
    pub task_id: String,
    pub generated_at: u64,
    pub goal: String,
    pub planner: String,
    pub llm_used: bool,
    pub steps: Vec<PlanStep>,
    pub likely_files: Vec<String>,
    pub forbidden_paths: Vec<String>,
    pub risk_level: String,
    pub approval_required: bool,
    pub warnings: Vec<String>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlanStep {
    pub index: usize,
    pub title: String,
    pub detail: String,
    pub expected_files: Vec<String>,
    pub risk: String,
}
