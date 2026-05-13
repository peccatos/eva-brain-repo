use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct EvolutionCoreReadiness {
    #[serde(default)]
    pub runtime_green: bool,
    #[serde(default)]
    pub approved_release_candidate: bool,
    #[serde(default)]
    pub release_bundle_exists: bool,
    #[serde(default)]
    pub tui_hydration_ok: bool,
    #[serde(default)]
    pub metrics_truth_ok: bool,
    #[serde(default)]
    pub candidate_queue_truth_ok: bool,
    #[serde(default)]
    pub phase_16_allowed: bool,
    #[serde(default)]
    pub blockers: Vec<String>,
}
