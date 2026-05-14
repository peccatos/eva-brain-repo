use std::fs;
use std::path::PathBuf;

#[path = "evolution_test_support.rs"]
mod evolution_test_support;

use eva_runtime_with_task_validator::{run_fix, FixOnly, FixRequest, FixRiskCap, FixStatus};

#[test]
fn proof_pack_dry_run_stays_read_only() {
    let root = evolution_test_support::unique_evolution_root("fix-proof-pack");
    fs::create_dir_all(root.join("src")).expect("src");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname=\"fix_proof_pack\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
    )
    .expect("cargo");
    fs::write(root.join("src/lib.rs"), "pub fn probe() -> bool { true }\n").expect("lib");
    let report = run_fix(FixRequest {
        fix_id: unique_fix_id("proof-pack"),
        target_path: root.clone(),
        dry_run: true,
        apply: false,
        only: Some(FixOnly::Ci),
        max_files: 3,
        risk_cap: FixRiskCap::Low,
        no_llm: true,
        provider: Some("rule_based".to_string()),
        evidence_dir: PathBuf::from(".eva/fix"),
    })
    .expect("fix report");
    assert_eq!(report.status, FixStatus::ProposalCreated);
    assert!(!report.source_mutation);
    assert!(report.evidence_written);
    assert!(root.join(".eva/fix").exists());
    assert!(!root.join(".github/workflows/rust-ci.yml").exists());
    evolution_test_support::remove_root(&root);
}

#[test]
fn proof_pack_documentation_exists() {
    assert!(std::path::Path::new("docs/phase_21_eve_fix.md").exists());
}

fn unique_fix_id(name: &str) -> String {
    format!(
        "fix-test-{}-{}",
        std::process::id(),
        name.replace(|ch: char| !ch.is_ascii_alphanumeric(), "-")
    )
}
