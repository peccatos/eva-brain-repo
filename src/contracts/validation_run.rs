use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentValidationStatus {
    Passed,
    Failed,
    Partial,
    NotRun,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRun {
    pub validation_id: String,
    pub task_id: Option<String>,
    pub proposal_id: Option<String>,
    pub status: AgentValidationStatus,
    pub started_at: u64,
    pub finished_at: u64,
    pub commands: Vec<ValidationCommandResult>,
    pub warnings: Vec<String>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationCommandResult {
    pub command: String,
    pub exit_code: Option<i32>,
    pub success: bool,
    pub stdout_tail: String,
    pub stderr_tail: String,
}
