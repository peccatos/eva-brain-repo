use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FuturePhaseEntry {
    #[serde(default)]
    pub phase: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub allowed_now: bool,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FuturePhaseRegistry {
    #[serde(default)]
    pub generated_at: u64,
    #[serde(default)]
    pub entries: Vec<FuturePhaseEntry>,
}
