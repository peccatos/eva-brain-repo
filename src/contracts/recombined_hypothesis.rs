use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecombinedHypothesis {
    pub hypothesis_id: String,
    pub source_patterns: Vec<String>,
    pub avoided_risks: Vec<String>,
    pub target_objective: String,
    pub suggested_mutation_kind: String,
    pub suggested_target: String,
    pub reason_ru: String,
    pub expected_gain: f32,
    pub estimated_risk: f32,
    pub confidence: f32,
}
