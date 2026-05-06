use serde::{Deserialize, Serialize};

use crate::contracts::EvolutionStatus;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvolutionReport {
    pub run_id: String,
    pub status: EvolutionStatus,
    pub goal_ru: String,
    pub selected_plan_ru: String,
    pub mutation_ru: String,
    pub target_file: String,
    pub mutation_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hypothesis_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_patterns: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub avoided_risks: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recombination_reason_ru: Option<String>,
    pub sandbox_ru: String,
    pub checks_ru: String,
    pub score_ru: String,
    pub candidate_ru: String,
    pub replay_ru: String,
    #[serde(default)]
    pub replay_status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_checked_at: Option<u64>,
    pub risk_ru: String,
    pub next_step_ru: String,
}
