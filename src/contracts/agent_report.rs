use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentReport {
    pub report_id: String,
    pub task_id: String,
    pub generated_at: u64,
    pub goal: String,
    pub task_status: String,
    pub inspection_id: Option<String>,
    pub plan_id: Option<String>,
    pub proposal_id: Option<String>,
    pub approval_id: Option<String>,
    pub apply_id: Option<String>,
    pub validation_id: Option<String>,
    pub files_changed: Vec<String>,
    pub validation_status: String,
    pub summary: String,
    pub risks: Vec<String>,
    pub next_actions: Vec<String>,
    pub warnings: Vec<String>,
    pub blockers: Vec<String>,
}
