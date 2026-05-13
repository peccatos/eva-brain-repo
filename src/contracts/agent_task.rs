use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentTaskStatus {
    Created,
    Inspected,
    Planned,
    Proposed,
    Approved,
    Applied,
    Validated,
    Reported,
    Completed,
    Blocked,
    Failed,
}

impl Default for AgentTaskStatus {
    fn default() -> Self {
        Self::Created
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    pub task_id: String,
    pub goal: String,
    pub status: AgentTaskStatus,
    pub scope: Vec<String>,
    pub forbidden_paths: Vec<String>,
    pub risk_level: String,
    pub approval_required: bool,
    pub created_at: u64,
    pub updated_at: u64,
    pub inspection_id: Option<String>,
    pub plan_id: Option<String>,
    pub proposal_id: Option<String>,
    pub approval_id: Option<String>,
    pub apply_id: Option<String>,
    pub validation_id: Option<String>,
    pub report_id: Option<String>,
    pub pr_summary_id: Option<String>,
    pub warnings: Vec<String>,
    pub blockers: Vec<String>,
}
