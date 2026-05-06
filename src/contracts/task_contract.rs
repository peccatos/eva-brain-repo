use serde::{Deserialize, Serialize};

use crate::contracts::{MutationKind, MutationObjective};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeniedMutationKind {
    DeleteCode,
    RewriteFunction,
    FreeDiff,
    DependencyAdd,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskContract {
    pub task_id: String,
    pub title_ru: String,
    pub goal_ru: String,
    pub allowed_targets: Vec<String>,
    pub forbidden_targets: Vec<String>,
    pub preferred_objectives: Vec<MutationObjective>,
    pub allowed_mutation_kinds: Vec<MutationKind>,
    pub denied_mutation_kinds: Vec<DeniedMutationKind>,
    pub cycles: usize,
    pub require_replay: bool,
    pub require_benchmark: bool,
    pub require_russian_report: bool,
    pub auto_promote: bool,
    pub max_risk: f32,
    pub min_score: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_corpus_id: Option<String>,
    pub created_at: u64,
}
