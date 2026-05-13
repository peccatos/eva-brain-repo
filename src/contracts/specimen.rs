use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SpecimenMetadata {
    pub specimen_id: String,
    pub kind: String,
    pub path: String,
    pub allowed_use: String,
    pub source_copy_allowed: bool,
    pub notes: Vec<String>,
}
