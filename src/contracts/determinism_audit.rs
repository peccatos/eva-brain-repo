use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DeterminismAuditReport {
    #[serde(default)]
    pub generated_at: u64,
    #[serde(default)]
    pub checked_documents: Vec<String>,
    #[serde(default)]
    pub missing_required_fields: Vec<String>,
    #[serde(default)]
    pub unstable_field_warnings: Vec<String>,
    #[serde(default)]
    pub full_source_content_warnings: Vec<String>,
    #[serde(default)]
    pub deterministic_enough: bool,
    #[serde(default)]
    pub recommendations_ru: Vec<String>,
}
