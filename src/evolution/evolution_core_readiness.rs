use crate::contracts::EvolutionCoreReadiness;
use crate::evolution::latest_release_id;
use crate::evolution::runtime_validation::load_or_build_runtime_validation;

pub fn build_evolution_core_readiness(
    project_root: &str,
    memory_root: &str,
) -> Result<EvolutionCoreReadiness, String> {
    let validation = load_or_build_runtime_validation(project_root, memory_root)?;
    let runtime_green = validation.status == "green";
    let approved_release_candidate = validation.approved_release_candidate.is_some();
    let release_bundle_exists =
        validation.release_bundle.is_some() || latest_release_id(memory_root)?.is_some();
    let tui_hydration_ok = true;
    let metrics_truth_ok = !validation.metrics_summary.is_empty();
    let candidate_queue_truth_ok = !validation.candidate_queue_summary.is_empty();

    let mut blockers = Vec::new();
    if !runtime_green {
        blockers.push(format!("runtime_validation_status={}", validation.status));
    }
    if !approved_release_candidate {
        blockers.push("approved_release_candidate_missing".to_string());
    }
    if !release_bundle_exists {
        blockers.push("release_bundle_missing".to_string());
    }

    Ok(EvolutionCoreReadiness {
        runtime_green,
        approved_release_candidate,
        release_bundle_exists,
        tui_hydration_ok,
        metrics_truth_ok,
        candidate_queue_truth_ok,
        phase_16_allowed: runtime_green,
        blockers,
    })
}
