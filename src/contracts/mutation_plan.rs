use serde::{Deserialize, Serialize};

use crate::contracts::mutation::MutationKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationObjective {
    ImproveReliability,
    ReduceRuntimeCost,
    ImproveScoring,
    ImproveValidation,
    ImproveReplayability,
    ImproveGraphMemory,
    ReduceStorage,
    ImproveTests,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MutationPlan {
    pub id: String,
    pub objective: MutationObjective,
    pub target_file: String,
    pub mutation_kind: MutationKind,
    pub reason: String,
    pub expected_gain: f32,
    pub estimated_risk: f32,
    pub evidence_weight: f32,
    pub graph_evidence: Vec<String>,
}
