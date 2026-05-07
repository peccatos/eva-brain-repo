use std::fs;
use std::path::{Path, PathBuf};

#[path = "evolution_test_support.rs"]
mod evolution_test_support;

use eva_runtime_with_task_validator::contracts::EvolutionStatus;
use eva_runtime_with_task_validator::evolution::{CandidateSummary, ReplayResult};
use eva_runtime_with_task_validator::{
    approve_candidate, build_artifact_audit, build_determinism_audit, build_preflight_gate,
    build_release_bundle, build_release_health, print_artifact_audit, print_future_phases,
    print_operator_runbook, print_proof_json, print_proof_report, print_release_ledger_json,
    record_release_attempt, run_demo, EvolutionReport, MutationContract, MutationKind,
    RuntimeCliCommand,
};
use serde_json::Value;

#[test]
fn release_health_reports_safe_metadata_and_auto_promote_false() {
    let root = temp_runtime_root("phase95-health");
    seed_candidate(
        &root,
        CandidateFixture::useful("health-candidate", 2_100_000_000),
    );
    let memory = root.join("memory");
    approve_candidate(
        root.to_str().unwrap(),
        memory.to_str().unwrap(),
        "health-candidate",
        "safe",
    )
    .expect("approve");

    let health = build_release_health(root.to_str().unwrap(), memory.to_str().unwrap())
        .expect("release health");
    assert!(health.release_runtime_support);
    assert!(!health.auto_promote);
    assert!(health.operator_approval_required);
    assert_eq!(health.ready_count, 1);
    assert_ne!(health.health_grade, "red");

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn artifact_audit_reports_runtime_paths_without_deleting() {
    let root = temp_runtime_root("phase95-artifacts");
    fs::create_dir_all(root.join("memory/releases/bundles")).expect("release dir");
    fs::write(root.join("memory/releases/bundles/demo.json"), "{}").expect("artifact");

    let audit = build_artifact_audit(root.to_str().unwrap()).expect("audit");
    assert!(audit
        .checked_paths
        .contains(&"memory/releases/".to_string()));
    assert!(root.join("memory/releases/bundles/demo.json").exists());
    assert!(!audit.should_fail_release);
    assert!(print_artifact_audit(root.to_str().unwrap())
        .expect("audit text")
        .contains("artifact_audit"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn artifact_audit_detects_sandbox_leak() {
    let root = temp_runtime_root("phase95-sandbox-leak");
    fs::create_dir_all(root.join("sandboxes/leak-run")).expect("leak");

    let audit = build_artifact_audit(root.to_str().unwrap()).expect("audit");
    assert!(!audit.sandbox_leaks.is_empty());
    assert!(audit.should_fail_release);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn determinism_audit_rejects_full_source_content_markers() {
    let root = temp_runtime_root("phase95-determinism-source");
    fs::create_dir_all(root.join("memory/releases/manifests")).expect("manifests");
    fs::write(
        root.join("memory/releases/manifests/bad.manifest.json"),
        r#"{
  "release_id": "release-bad",
  "source_run_id": "run-bad",
  "target_file": "tests/evolution_generated_tests.rs",
  "mutation_kind": "addunittest",
  "mutation_class": "useful",
  "replay_status": "ok",
  "approved": true,
  "auto_promote": false,
  "source_mutated": false,
  "rollback_available": true,
  "changelog_available": true,
  "created_at": 1,
  "copied_source": "pub fn copied_source_marker() {}"
}"#,
    )
    .expect("manifest");

    let audit = build_determinism_audit(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("determinism audit");
    assert!(!audit.full_source_content_warnings.is_empty());
    assert!(!audit.deterministic_enough);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn preflight_gate_fails_on_sandbox_leak() {
    let root = temp_runtime_root("phase95-gate-leak");
    fs::create_dir_all(root.join("sandboxes/leak-run")).expect("leak");
    let gate = build_preflight_gate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("gate");
    assert_eq!(gate.gate_status, "fail");
    assert!(gate.blockers.contains(&"sandbox_leaks".to_string()));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn preflight_gate_warns_when_no_release_candidate() {
    let root = temp_runtime_root("phase95-gate-no-candidate");
    let gate = build_preflight_gate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("gate");
    assert_eq!(gate.gate_status, "warn");
    assert!(gate
        .warnings
        .contains(&"no_approved_release_candidate".to_string()));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn release_ledger_appends_metadata_only_record() {
    let root = temp_runtime_root("phase95-ledger");
    seed_candidate(
        &root,
        CandidateFixture::useful("ledger-candidate", 2_100_000_100),
    );
    let memory = root.join("memory");
    approve_candidate(
        root.to_str().unwrap(),
        memory.to_str().unwrap(),
        "ledger-candidate",
        "safe",
    )
    .expect("approve");
    let bundle = build_release_bundle(
        root.to_str().unwrap(),
        memory.to_str().unwrap(),
        "ledger-candidate",
    )
    .expect("bundle");

    let record = record_release_attempt(
        root.to_str().unwrap(),
        memory.to_str().unwrap(),
        &bundle.release_id,
    )
    .expect("record");
    assert_eq!(record.release_id, bundle.release_id);
    let ledger_json = print_release_ledger_json(memory.to_str().unwrap()).expect("ledger json");
    let ledger: Vec<Value> = serde_json::from_str(&ledger_json).expect("parse ledger");
    assert_eq!(ledger.len(), 1);
    assert!(!ledger_json.contains("pub fn "));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn future_phase_registry_is_static_and_non_executing() {
    let first = print_future_phases();
    let second = print_future_phases();
    assert_eq!(first, second);
    assert!(first.contains("Phase 10.0"));
    assert!(first.contains("allowed_now=false"));
    assert!(first.contains("Controlled Self-Modification Review Runtime"));
}

#[test]
fn operator_runbook_prints_next_actions() {
    let root = temp_runtime_root("phase95-runbook");
    let runbook = print_operator_runbook(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("runbook");
    assert!(runbook.contains("Следующая команда"));
    assert!(runbook.contains("auto_promote=false"));
    assert!(runbook.contains("Future phases allowed_now=false"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn proof_report_includes_phase95_capabilities() {
    let root = temp_runtime_root("phase95-proof");
    let report = print_proof_report(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("proof");
    assert!(report.contains("release_health_support=true"));
    assert!(report.contains("artifact_audit_support=true"));
    assert!(report.contains("determinism_audit_support=true"));
    assert!(report.contains("preflight_gate_v2_support=true"));
    assert!(report.contains("release_ledger_support=true"));
    assert!(report.contains("future_phase_registry_support=true"));
    assert!(report.contains("operator_runbook_support=true"));
    let json = print_proof_json(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("proof json");
    let value: Value = serde_json::from_str(&json).expect("parse proof");
    assert_eq!(value["release_health_support"].as_bool(), Some(true));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn demo_includes_governance_release_and_phase95_summary() {
    let root = temp_runtime_root("phase95-demo");
    let output = run_demo(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("demo");
    assert!(output.contains("governance_status"));
    assert!(output.contains("release_status"));
    assert!(output.contains("release_health"));
    assert!(output.contains("preflight_gate"));
    assert!(output.contains("artifact_audit"));
    assert!(!root.join("sandboxes").exists());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn runtime_daemon_json_config_test_uses_isolated_root_or_no_quota_sensitive_path() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".eva-runtime-tests");
    assert!(root.ends_with(".eva-runtime-tests"));
    let command = RuntimeCliCommand::parse_from_iter([
        "--serve".to_string(),
        "--config".to_string(),
        root.join("nonexistent.json").display().to_string(),
    ]);
    assert!(command
        .expect_err("missing config")
        .contains(".eva-runtime-tests"));
}

#[derive(Clone)]
struct CandidateFixture {
    run_id: String,
    kind: MutationKind,
    mutation_class: String,
    useful_change: bool,
    replay_status: String,
    timestamp_unix: u64,
}

impl CandidateFixture {
    fn useful(run_id: &str, timestamp_unix: u64) -> Self {
        Self {
            run_id: run_id.to_string(),
            kind: MutationKind::AddUnitTest,
            mutation_class: "useful".to_string(),
            useful_change: true,
            replay_status: "ok".to_string(),
            timestamp_unix,
        }
    }
}

fn temp_runtime_root(name: &str) -> PathBuf {
    let root = evolution_test_support::unique_evolution_root(name);
    fs::create_dir_all(root.join("src")).expect("src");
    fs::create_dir_all(root.join("tests")).expect("tests");
    fs::create_dir_all(root.join("memory")).expect("memory");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"phase95_temp\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\ndoctest = false\n",
    )
    .expect("cargo");
    fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("main");
    fs::write(root.join("src/lib.rs"), "pub fn probe() -> bool { true }\n").expect("lib");
    fs::write(
        root.join("tests/evolution_generated_tests.rs"),
        "#[test]\nfn existing_test() { assert!(true); }\n",
    )
    .expect("tests");
    seed_autonomy_memory(&root);
    root
}

fn seed_candidate(root: &Path, fixture: CandidateFixture) {
    fs::create_dir_all(root.join("memory/candidates")).expect("candidates");
    fs::create_dir_all(root.join("memory/reports")).expect("reports");
    fs::create_dir_all(root.join("memory/replays")).expect("replays");
    fs::write(root.join("memory/regressions.json"), "[]").expect("regressions");
    fs::write(root.join("memory/success_patterns.json"), "[]").expect("success");
    let mutation_kind = format!("{:?}", fixture.kind).to_ascii_lowercase();
    let summary = CandidateSummary {
        run_id: fixture.run_id.clone(),
        mutation_id: format!("mutation-{}", fixture.run_id),
        mutation_digest: format!("digest-{}", fixture.run_id),
        status: EvolutionStatus::Candidate,
        target_file: "tests/evolution_generated_tests.rs".to_string(),
        mutation_kind: mutation_kind.clone(),
        risk: 0.10,
        score: 9.5,
        useful_change: fixture.useful_change,
        non_candidate_reason: None,
        duplicate_rejected: false,
        regression_penalty: 0.0,
        success_bonus: 0.0,
        cargo_check_ok: true,
        cargo_test_ok: true,
        cargo_run_ok: true,
        stdout_digest: String::new(),
        stderr_digest: String::new(),
        stderr_tail: String::new(),
        timestamp_unix: fixture.timestamp_unix,
    };
    let mutation = MutationContract {
        id: summary.mutation_id.clone(),
        kind: fixture.kind,
        target_file: summary.target_file.clone(),
        search: None,
        replace: None,
        append: Some(
            "#[test]\nfn eva_generated_phase95_fixture() { assert!(true); }\n".to_string(),
        ),
        reason: "fixture".to_string(),
        expected_gain: 0.5,
        risk: summary.risk,
    };
    let report = EvolutionReport {
        run_id: fixture.run_id.clone(),
        status: EvolutionStatus::Candidate,
        goal_ru: "fixture".to_string(),
        selected_plan_ru: "fixture".to_string(),
        mutation_ru: "fixture".to_string(),
        target_file: summary.target_file.clone(),
        mutation_kind,
        hypothesis_id: None,
        source_patterns: Vec::new(),
        avoided_risks: Vec::new(),
        recombination_reason_ru: None,
        portfolio_reason_ru: None,
        selected_strategy: Some("ReplaySafety".to_string()),
        policy_reason_ru: Some("fixture".to_string()),
        mutation_class: fixture.mutation_class.clone(),
        hygiene_warning_ru: None,
        diversity_bonus: 0.0,
        saturation_penalty: 0.0,
        repeated_target_penalty: 0.0,
        final_recombination_score: 0.0,
        strategy_bonus: 0.0,
        strategy_saturation_penalty: 0.0,
        quality_bonus: 0.0,
        novelty_score: 0.0,
        useful_delta_score: 0.0,
        duplicate_suppression_score: 0.0,
        regression_avoidance_score: 0.0,
        coverage_proxy_score: 0.0,
        quality_score: 0.9,
        final_strategy_score: 0.9,
        sandbox_ru: "ok".to_string(),
        checks_ru: "ok".to_string(),
        score_ru: "ok".to_string(),
        candidate_ru: "ok".to_string(),
        replay_ru: "ok".to_string(),
        replay_status: fixture.replay_status.clone(),
        replay_checked_at: Some(fixture.timestamp_unix),
        risk_ru: "ok".to_string(),
        next_step_ru: "ok".to_string(),
    };
    let replay = ReplayResult {
        run_id: fixture.run_id.clone(),
        replay_status: EvolutionStatus::Candidate,
        score: 9.5,
        matches_stored_summary: true,
        cargo_check_ok: true,
        cargo_test_ok: true,
        cargo_run_ok: true,
        stdout_digest: String::new(),
        stderr_digest: String::new(),
        stderr_tail: String::new(),
        sandbox_destroyed: true,
        timestamp_unix: fixture.timestamp_unix,
    };
    fs::write(
        root.join("memory/candidates")
            .join(format!("{}.summary.json", fixture.run_id)),
        serde_json::to_string_pretty(&summary).expect("summary"),
    )
    .expect("summary");
    fs::write(
        root.join("memory/candidates")
            .join(format!("{}.mutation.json", fixture.run_id)),
        serde_json::to_string_pretty(&mutation).expect("mutation"),
    )
    .expect("mutation");
    fs::write(
        root.join("memory/reports")
            .join(format!("{}.report.json", fixture.run_id)),
        serde_json::to_string_pretty(&report).expect("report"),
    )
    .expect("report");
    fs::write(
        root.join("memory/reports")
            .join(format!("{}.ru.md", fixture.run_id)),
        "fixture report",
    )
    .expect("report md");
    fs::write(
        root.join("memory/replays")
            .join(format!("{}.json", fixture.run_id)),
        serde_json::to_string_pretty(&replay).expect("replay"),
    )
    .expect("replay");
}

fn seed_autonomy_memory(root: &Path) {
    fs::create_dir_all(root.join("memory/replays")).expect("replays");
    fs::write(root.join("memory/regressions.json"), "[]").expect("regressions");
    fs::write(root.join("memory/success_patterns.json"), "[]").expect("success");
    let mut lines = Vec::new();
    for index in 0..12 {
        lines.push(format!(
            "{{\"run_id\":\"seed-{index}\",\"plan_id\":null,\"hypothesis_id\":null,\"objective\":\"ImproveTests\",\"graph_evidence\":[],\"recombined_source_patterns\":[],\"recombined_avoided_risks\":[],\"recombination_reason_ru\":null,\"portfolio_reason_ru\":null,\"selected_strategy\":null,\"policy_reason_ru\":null,\"mutation_class\":\"useful\",\"hygiene_warning_ru\":null,\"diversity_bonus\":0.0,\"saturation_penalty\":0.0,\"repeated_target_penalty\":0.0,\"final_recombination_score\":0.0,\"strategy_bonus\":0.0,\"strategy_saturation_penalty\":0.0,\"quality_bonus\":0.0,\"novelty_score\":0.0,\"useful_delta_score\":0.0,\"duplicate_suppression_score\":0.0,\"regression_avoidance_score\":0.0,\"coverage_proxy_score\":0.0,\"quality_score\":0.9,\"final_strategy_score\":0.9,\"mutation_id\":\"m-{index}\",\"mutation_digest\":\"d-{index}\",\"status\":\"candidate\",\"target_file\":\"tests/evolution_generated_tests.rs\",\"mutation_kind\":\"addunittest\",\"risk\":0.10,\"score\":8.50,\"useful_change\":true,\"non_candidate_reason\":null,\"duplicate_rejected\":false,\"regression_penalty\":0.0,\"success_bonus\":0.0,\"cargo_check_ok\":true,\"cargo_test_ok\":true,\"cargo_run_ok\":true,\"retained_in_core\":false,\"sandbox_destroyed\":true,\"stdout_digest\":\"\",\"stderr_digest\":\"\",\"stderr_tail\":\"\",\"timestamp_unix\":{}}}",
            index + 1
        ));
    }
    fs::write(root.join("memory/evolution.jsonl"), lines.join("\n") + "\n").expect("evolution");
    for index in 0..4 {
        let replay = ReplayResult {
            run_id: format!("seed-{index}"),
            replay_status: EvolutionStatus::Candidate,
            score: 8.5,
            matches_stored_summary: true,
            cargo_check_ok: true,
            cargo_test_ok: true,
            cargo_run_ok: true,
            stdout_digest: String::new(),
            stderr_digest: String::new(),
            stderr_tail: String::new(),
            sandbox_destroyed: true,
            timestamp_unix: index + 1,
        };
        fs::write(
            root.join("memory/replays")
                .join(format!("seed-{index}.json")),
            serde_json::to_string_pretty(&replay).expect("replay"),
        )
        .expect("seed replay");
    }
    fs::write(
        root.join("memory/metrics.json"),
        r#"{"total_runs":12,"passed_runs":12,"failed_runs":0,"candidate_count":12,"replay_passed":4,"promoted_count":0,"average_score":8.5,"last_run_id":"seed-11"}"#,
    )
    .expect("metrics");
}
