use crate::contracts::{FuturePhaseEntry, FuturePhaseRegistry};
use crate::evolution::memory;

pub fn build_future_phase_registry() -> FuturePhaseRegistry {
    FuturePhaseRegistry {
        generated_at: memory::now_unix(),
        entries: vec![
            FuturePhaseEntry {
                phase: "10.0".to_string(),
                name: "CI/PR Integration Runtime".to_string(),
                status: "planned".to_string(),
                allowed_now: false,
                reason: "requires stable local release gate first".to_string(),
            },
            FuturePhaseEntry {
                phase: "11.0".to_string(),
                name: "Daemonized Operator Service".to_string(),
                status: "planned".to_string(),
                allowed_now: false,
                reason: "requires release health and artifact audit stability".to_string(),
            },
            FuturePhaseEntry {
                phase: "12.0".to_string(),
                name: "External Repository Patch Pipeline".to_string(),
                status: "planned".to_string(),
                allowed_now: false,
                reason: "requires stronger sandbox and rollback proofs".to_string(),
            },
            FuturePhaseEntry {
                phase: "13.0".to_string(),
                name: "Controlled Self-Modification Review Runtime".to_string(),
                status: "planned".to_string(),
                allowed_now: false,
                reason: "requires governance, release, rollback, and reproducibility guarantees"
                    .to_string(),
            },
        ],
    }
}

pub fn print_future_phases() -> String {
    build_future_phase_registry()
        .entries
        .iter()
        .map(|entry| {
            format!(
                "Phase {}: {} status={} allowed_now={} reason={}",
                entry.phase, entry.name, entry.status, entry.allowed_now, entry.reason
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn print_future_phases_json() -> Result<String, String> {
    serde_json::to_string_pretty(&build_future_phase_registry())
        .map_err(|error| format!("failed to serialize future phase registry: {error}"))
}
