use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PrSummary {
    pub pr_summary_id: String,
    pub task_id: String,
    pub generated_at: u64,
    pub title: String,
    pub body: String,
    pub validation_checklist: Vec<String>,
    pub risk_notes: Vec<String>,
}
