use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProposalStatus {
    Draft,
    AwaitingApproval,
    Approved,
    Rejected,
    Applied,
    Failed,
    Refused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchProposal {
    pub proposal_id: String,
    pub task_id: String,
    pub plan_id: String,
    pub status: ProposalStatus,
    pub created_at: u64,
    pub updated_at: u64,
    pub goal: String,
    pub summary: String,
    pub proposer: String,
    pub llm_used: bool,
    pub files_to_change: Vec<String>,
    pub forbidden_paths: Vec<String>,
    pub risk_level: String,
    pub approval_required: bool,
    pub approved: bool,
    pub approved_at: Option<u64>,
    pub patch_ops: Vec<PatchOp>,
    pub warnings: Vec<String>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchOp {
    pub path: String,
    pub op: PatchOperationKind,
    pub description: String,
    pub content: Option<String>,
    pub find: Option<String>,
    pub replace: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PatchOperationKind {
    CreateFile,
    AppendFile,
    ReplaceFileIfExists,
    ReplaceExactText,
}
