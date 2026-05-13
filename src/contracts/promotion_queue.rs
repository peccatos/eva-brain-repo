use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateState {
    Ready,
    Blocked,
    Quarantined,
    Stale,
    Legacy,
    Duplicate,
    Unreplayable,
    AlreadyPromoted,
    Unknown,
}

impl Default for CandidateState {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PromotionQueueItem {
    #[serde(default)]
    pub run_id: String,
    #[serde(default)]
    pub mutation_kind: String,
    #[serde(default)]
    pub mutation_class: String,
    #[serde(default)]
    pub target_file: String,
    #[serde(default)]
    pub score: f32,
    #[serde(default)]
    pub risk: f32,
    #[serde(default)]
    pub replay_status: String,
    #[serde(default)]
    pub promotion_state: String,
    #[serde(default)]
    pub promotion_allowed: bool,
    #[serde(default)]
    pub promotion_blockers: Vec<String>,
    #[serde(default)]
    pub report_path: String,
    #[serde(default)]
    pub lifecycle_state: String,
    #[serde(default)]
    pub candidate_state: CandidateState,
    #[serde(default)]
    pub candidate_state_reason: String,
    #[serde(default)]
    pub cargo_test_ok: Option<bool>,
    #[serde(default)]
    pub cargo_run_ok: Option<bool>,
    #[serde(default)]
    pub duplicate_rejected: bool,
    #[serde(default)]
    pub promoted: bool,
    #[serde(default)]
    pub reason_ru: String,
    #[serde(default)]
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PromotionQueue {
    #[serde(default)]
    pub items: Vec<PromotionQueueItem>,
    #[serde(default)]
    pub generated_at: u64,
    #[serde(default)]
    pub summary: CandidateQueueSummary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CandidateQueueSummary {
    #[serde(default)]
    pub candidate_count: usize,
    #[serde(default)]
    pub ready_candidates: usize,
    #[serde(default)]
    pub blocked_candidates: usize,
    #[serde(default)]
    pub quarantined_candidates: usize,
    #[serde(default)]
    pub legacy_candidates: usize,
    #[serde(default)]
    pub duplicate_candidates: usize,
    #[serde(default)]
    pub unreplayable_candidates: usize,
    #[serde(default)]
    pub already_promoted_candidates: usize,
    #[serde(default)]
    pub unknown_candidates: usize,
}
