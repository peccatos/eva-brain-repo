use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentApproval {
    pub approval_id: String,
    pub proposal_id: String,
    pub task_id: String,
    pub approved: bool,
    pub approved_at: u64,
    pub approved_by: String,
    pub reason: String,
    pub warnings: Vec<String>,
    pub blockers: Vec<String>,
}
