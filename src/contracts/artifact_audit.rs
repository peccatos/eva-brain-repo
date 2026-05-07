use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ArtifactAuditReport {
    #[serde(default)]
    pub generated_at: u64,
    #[serde(default)]
    pub checked_paths: Vec<String>,
    #[serde(default)]
    pub tracked_runtime_artifacts: Vec<String>,
    #[serde(default)]
    pub untracked_runtime_artifacts: Vec<String>,
    #[serde(default)]
    pub ignored_runtime_artifacts: Vec<String>,
    #[serde(default)]
    pub sandbox_leaks: Vec<String>,
    #[serde(default)]
    pub should_fail_release: bool,
    #[serde(default)]
    pub recommendations_ru: Vec<String>,
}
