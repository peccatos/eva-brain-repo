use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ReleaseManifest {
    #[serde(default)]
    pub release_id: String,
    #[serde(default)]
    pub source_run_id: String,
    #[serde(default)]
    pub target_file: String,
    #[serde(default)]
    pub mutation_kind: String,
    #[serde(default)]
    pub mutation_class: String,
    #[serde(default)]
    pub replay_status: String,
    #[serde(default)]
    pub approved: bool,
    #[serde(default)]
    pub auto_promote: bool,
    #[serde(default)]
    pub source_mutated: bool,
    #[serde(default)]
    pub rollback_available: bool,
    #[serde(default)]
    pub changelog_available: bool,
    #[serde(default)]
    pub created_at: u64,
}
