use std::path::Path;

use crate::contracts::{RuntimeCliCommandContract, RuntimeCliContractReport};
use crate::evolution::memory;

pub fn build_runtime_cli_contract(memory_root: &str) -> Result<RuntimeCliContractReport, String> {
    let report = RuntimeCliContractReport {
        generated_at: memory::now_unix(),
        commands: vec![
            entry(
                "--eva-status",
                "Print compact EVA operator status",
                "read_only",
            ),
            entry(
                "--operator-console",
                "Print combined operator console",
                "read_only",
            ),
            entry(
                "--proof-report",
                "Print proof markdown report",
                "metadata_only",
            ),
            entry("--proof-json", "Print proof JSON", "metadata_only"),
            entry(
                "--capability-policy",
                "Print capability policy JSON",
                "metadata_only",
            ),
            entry(
                "--trust-decision",
                "Print trust decision JSON",
                "blocked_if_untrusted",
            ),
            entry(
                "--workspace-snapshot",
                "Build workspace snapshot JSON",
                "metadata_only",
            ),
            entry(
                "--evidence-bundle",
                "Build evidence bundle JSON",
                "metadata_only",
            ),
            entry(
                "--recovery-manifest",
                "Build recovery manifest JSON",
                "metadata_only",
            ),
            entry(
                "--preflight-gate-v3",
                "Print trust and recovery preflight gate v3",
                "blocked_if_untrusted",
            ),
            entry(
                "--trust-proof-report",
                "Print trust proof markdown report",
                "metadata_only",
            ),
            entry(
                "--release-status",
                "Print release runtime status",
                "read_only",
            ),
            entry(
                "--release-health",
                "Print release health report",
                "metadata_only",
            ),
            entry(
                "--artifact-audit",
                "Print artifact audit report",
                "validation_only",
            ),
            entry(
                "--determinism-audit",
                "Print determinism audit report",
                "validation_only",
            ),
            entry("--ops-status", "Print operations status", "read_only"),
            entry(
                "--ops-json",
                "Print operations status JSON",
                "metadata_only",
            ),
            entry(
                "tui / --tui",
                "Open read-only operator terminal dashboard",
                "read_only",
            ),
            entry(
                "status / --status",
                "Print runtime validation status",
                "validation_only",
            ),
            entry(
                "--runtime-candidate",
                "Build runtime v1.0 candidate manifest",
                "metadata_only",
            ),
            entry(
                "--runtime-validation",
                "Build runtime validation report",
                "validation_only",
            ),
            entry(
                "--runtime-service",
                "Print local operator service metadata",
                "metadata_only",
            ),
            entry(
                "--runtime-cli-contract",
                "Print stable runtime CLI contract",
                "metadata_only",
            ),
            entry(
                "--release-approve <RUN_ID>",
                "Approve a ready replay-ok candidate for release metadata",
                "blocked_if_untrusted",
            ),
            entry(
                "--final-rc-report",
                "Print final runtime v1.0 candidate markdown report",
                "metadata_only",
            ),
        ],
    };
    memory::write_json(
        Path::new(memory_root)
            .join("runtime_service")
            .join("runtime_cli_contract.json"),
        &report,
    )?;
    Ok(report)
}

pub fn print_runtime_cli_contract(memory_root: &str) -> Result<String, String> {
    serde_json::to_string_pretty(&build_runtime_cli_contract(memory_root)?)
        .map_err(|error| format!("failed to serialize runtime CLI contract: {error}"))
}

fn entry(command: &str, purpose: &str, safety_class: &str) -> RuntimeCliCommandContract {
    RuntimeCliCommandContract {
        command: command.to_string(),
        purpose: purpose.to_string(),
        mutates_source: false,
        mutates_external_repo: false,
        requires_operator_approval: matches!(safety_class, "blocked_if_untrusted"),
        safety_class: safety_class.to_string(),
    }
}
