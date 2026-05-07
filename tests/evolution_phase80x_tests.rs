use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "evolution_test_support.rs"]
mod evolution_test_support;

use eva_runtime_with_task_validator::contracts::EvolutionStatus;
use eva_runtime_with_task_validator::evolution::{CandidateSummary, ReplayResult};
use eva_runtime_with_task_validator::graph::{GraphEdge, GraphNode};
use eva_runtime_with_task_validator::{
    approval_log, approve_candidate, defer_candidate, governance_status, governance_trust_gate,
    print_proof_snapshot, print_proof_snapshot_json, print_release_proposal,
    print_release_proposal_json, promote_approved_candidate, reject_candidate, run_demo,
    EvolutionGraph, EvolutionLogEntry, EvolutionReport, MutationContract, MutationKind,
};
use serde_json::Value;

#[test]
fn approve_reject_defer_and_idempotence_work() {
    let root = temp_runtime_root("phase80-approval");
    seed_autonomy_memory(&root);
    seed_candidate_fixture(&root, CandidateFixture::ready("approve-run"));

    let approved = approve_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "approve-run",
        "manual ok",
    )
    .expect("approve");
    let approved_again = approve_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "approve-run",
        "manual ok",
    )
    .expect("approve again");
    assert_eq!(approved, approved_again);

    seed_candidate_fixture(&root, CandidateFixture::ready("reject-run"));
    seed_candidate_fixture(&root, CandidateFixture::ready("defer-run"));
    reject_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "reject-run",
        "not now",
    )
    .expect("reject");
    defer_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "defer-run",
        "later",
    )
    .expect("defer");

    let log = approval_log(root.join("memory").to_str().unwrap()).expect("log");
    assert_eq!(log.len(), 3);
    assert!(log.iter().any(|record| record.decision == "approve"));
    assert!(log.iter().any(|record| record.decision == "reject"));
    assert!(log.iter().any(|record| record.decision == "defer"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn approval_safety_rejects_invalid_candidates() {
    let root = temp_runtime_root("phase80-approval-safety");
    seed_autonomy_memory(&root);
    seed_candidate_fixture(&root, CandidateFixture::cosmetic("cosmetic-run"));
    seed_candidate_fixture(&root, CandidateFixture::unsafe_class("unsafe-run"));
    seed_candidate_fixture(&root, CandidateFixture::legacy("legacy-run"));
    seed_candidate_fixture(&root, CandidateFixture::already_promoted("promoted-run"));
    seed_candidate_fixture(&root, CandidateFixture::needs_replay("replay-run"));
    seed_candidate_fixture(&root, CandidateFixture::high_risk("high-risk-run"));

    assert!(approve_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "cosmetic-run",
        "x"
    )
    .is_err());
    assert!(approve_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "unsafe-run",
        "x"
    )
    .is_err());
    assert!(approve_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "legacy-run",
        "x"
    )
    .is_err());
    assert!(approve_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "promoted-run",
        "x"
    )
    .is_err());
    assert!(approve_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "replay-run",
        "x"
    )
    .is_err());
    assert!(approve_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "high-risk-run",
        "x"
    )
    .is_err());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn governance_trust_gate_handles_approval_rejection_defer_and_stale() {
    let root = temp_runtime_root("phase80-trust-gate");
    seed_autonomy_memory(&root);
    seed_candidate_fixture(&root, CandidateFixture::ready("trust-ready"));
    seed_candidate_fixture(&root, CandidateFixture::ready("trust-reject"));
    seed_candidate_fixture(&root, CandidateFixture::ready("trust-defer"));
    seed_candidate_fixture(&root, CandidateFixture::ready("trust-stale"));
    seed_candidate_fixture(&root, CandidateFixture::ready("trust-forbidden"));

    approve_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "trust-ready",
        "ok",
    )
    .expect("approve ready");
    reject_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "trust-reject",
        "no",
    )
    .expect("reject");
    defer_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "trust-defer",
        "later",
    )
    .expect("defer");
    approve_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "trust-stale",
        "ok",
    )
    .expect("approve stale");
    approve_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "trust-forbidden",
        "ok",
    )
    .expect("approve forbidden initially");

    let ready_gate = governance_trust_gate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "trust-ready",
        false,
    )
    .expect("gate");
    assert!(ready_gate.allowed);

    let reject_gate = governance_trust_gate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "trust-reject",
        false,
    )
    .expect("gate");
    assert!(!reject_gate.allowed);
    assert!(reject_gate
        .blockers
        .contains(&"rejected_by_operator".to_string()));

    let defer_gate = governance_trust_gate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "trust-defer",
        false,
    )
    .expect("gate");
    assert!(!defer_gate.allowed);
    assert!(defer_gate
        .blockers
        .contains(&"deferred_by_operator".to_string()));

    mutate_candidate_score(&root, "trust-stale", 9.7);
    let stale_gate = governance_trust_gate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "trust-stale",
        false,
    )
    .expect("gate");
    assert!(!stale_gate.allowed);
    assert!(stale_gate.blockers.contains(&"approval_stale".to_string()));

    mutate_candidate_target(&root, "trust-forbidden", "src/main.rs");
    let forbidden_gate = governance_trust_gate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "trust-forbidden",
        false,
    )
    .expect("gate");
    assert!(!forbidden_gate.allowed);
    assert!(forbidden_gate
        .blockers
        .contains(&"forbidden_target".to_string()));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn promote_approved_requires_governance_pass_and_logs_event() {
    let root = temp_runtime_root("phase80-promote-approved");
    seed_autonomy_memory(&root);
    seed_candidate_fixture(&root, CandidateFixture::ready("promote-run"));
    seed_candidate_fixture(&root, CandidateFixture::ready("reject-run"));
    seed_candidate_fixture(&root, CandidateFixture::ready("defer-run"));

    assert!(promote_approved_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "promote-run"
    )
    .is_err());

    reject_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "reject-run",
        "no",
    )
    .expect("reject");
    assert!(promote_approved_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "reject-run"
    )
    .is_err());

    defer_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "defer-run",
        "later",
    )
    .expect("defer");
    assert!(promote_approved_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "defer-run"
    )
    .is_err());

    approve_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "promote-run",
        "safe",
    )
    .expect("approve");
    let status = promote_approved_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "promote-run",
    )
    .expect("promote approved");
    assert_eq!(status, "promotion_status: ok");

    let log = approval_log(root.join("memory").to_str().unwrap()).expect("approval log");
    assert!(log
        .iter()
        .any(|record| record.run_id == "promote-run" && record.decision == "promoted"));
    let evolution_log = fs::read_to_string(root.join("memory/evolution.jsonl")).expect("evolution");
    assert!(evolution_log.contains("\"retained_in_core\":true"));
    let governance = governance_status(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("gov");
    assert!(!governance.auto_promote);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn release_proposal_includes_only_approved_ready_candidates_and_is_deterministic() {
    let root = temp_runtime_root("phase80-release");
    seed_autonomy_memory(&root);
    seed_candidate_fixture(&root, CandidateFixture::ready("release-approve"));
    seed_candidate_fixture(&root, CandidateFixture::ready("release-reject"));
    seed_candidate_fixture(&root, CandidateFixture::ready("release-defer"));
    seed_candidate_fixture(
        &root,
        CandidateFixture::already_promoted("release-promoted"),
    );

    approve_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "release-approve",
        "ship it",
    )
    .expect("approve");
    reject_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "release-reject",
        "no",
    )
    .expect("reject");
    defer_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "release-defer",
        "later",
    )
    .expect("defer");

    let markdown = print_release_proposal(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("markdown");
    let json_a = print_release_proposal_json(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("json a");
    let json_b = print_release_proposal_json(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("json b");
    assert_eq!(json_a, json_b);
    assert!(markdown.contains("release-approve"));
    assert!(!markdown.contains("release-reject"));
    assert!(!markdown.contains("release-defer"));
    assert!(!markdown.contains("release-promoted"));

    let value: Value = serde_json::from_str(&json_a).expect("parse");
    let proposal_id = value["proposal_id"].as_str().expect("proposal id");
    assert!(root
        .join("memory/governance/release_proposals")
        .join(format!("{proposal_id}.json"))
        .exists());
    assert!(root
        .join("memory/governance/release_proposals")
        .join(format!("{proposal_id}.ru.md"))
        .exists());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn proof_snapshot_writes_files_and_counts() {
    let root = temp_runtime_root("phase80-proof-snapshot");
    seed_autonomy_memory(&root);
    seed_candidate_fixture(&root, CandidateFixture::ready("snapshot-approve"));
    seed_candidate_fixture(&root, CandidateFixture::ready("snapshot-reject"));
    seed_candidate_fixture(&root, CandidateFixture::ready("snapshot-defer"));

    approve_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "snapshot-approve",
        "ok",
    )
    .expect("approve");
    reject_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "snapshot-reject",
        "no",
    )
    .expect("reject");
    defer_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "snapshot-defer",
        "later",
    )
    .expect("defer");

    let markdown = print_proof_snapshot(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("snapshot md");
    let json = print_proof_snapshot_json(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("snapshot json");
    let value: Value = serde_json::from_str(&json).expect("parse snapshot");
    let snapshot_id = value["snapshot_id"].as_str().expect("snapshot id");
    assert!(markdown.contains("operator_approval_required=true"));
    assert_eq!(value["approved_count"].as_u64(), Some(1));
    assert_eq!(value["rejected_count"].as_u64(), Some(1));
    assert_eq!(value["deferred_count"].as_u64(), Some(1));
    assert_eq!(value["operator_approval_required"].as_bool(), Some(true));
    assert_eq!(value["auto_promote"].as_bool(), Some(false));
    assert!(root
        .join("memory/governance/proof_snapshots")
        .join(format!("{snapshot_id}.json"))
        .exists());
    assert!(root
        .join("memory/governance/proof_snapshots")
        .join(format!("{snapshot_id}.ru.md"))
        .exists());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn governance_safety_commands_do_not_create_sandbox_leaks_or_mutate_source() {
    let root = temp_runtime_root("phase80-safety");
    seed_autonomy_memory(&root);
    seed_candidate_fixture(&root, CandidateFixture::ready("safe-run"));
    let before_main = fs::read_to_string(root.join("src/main.rs")).expect("main");
    let before_lib = fs::read_to_string(root.join("src/lib.rs")).expect("lib");

    approve_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "safe-run",
        "ok",
    )
    .expect("approve");
    reject_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "safe-run",
        "later reject",
    )
    .expect("reject");
    defer_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "safe-run",
        "later defer",
    )
    .expect("defer");
    let _ = print_proof_snapshot(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("snapshot");
    let demo = run_demo(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("demo");

    assert!(demo.contains("governance_status:"));
    assert_eq!(
        before_main,
        fs::read_to_string(root.join("src/main.rs")).expect("main after")
    );
    assert_eq!(
        before_lib,
        fs::read_to_string(root.join("src/lib.rs")).expect("lib after")
    );
    assert!(!root.join("sandboxes").exists());

    fs::remove_dir_all(root).expect("cleanup");
}

#[derive(Clone)]
struct CandidateFixture {
    run_id: String,
    kind: MutationKind,
    mutation_class: String,
    replay_status: String,
    useful_change: bool,
    already_promoted: bool,
    risk: f32,
    target_file: String,
}

impl CandidateFixture {
    fn ready(run_id: &str) -> Self {
        Self {
            run_id: run_id.to_string(),
            kind: MutationKind::AddUnitTest,
            mutation_class: "useful".to_string(),
            replay_status: "ok".to_string(),
            useful_change: true,
            already_promoted: false,
            risk: 0.10,
            target_file: "tests/evolution_generated_tests.rs".to_string(),
        }
    }

    fn cosmetic(run_id: &str) -> Self {
        Self {
            run_id: run_id.to_string(),
            kind: MutationKind::AppendComment,
            mutation_class: "cosmetic".to_string(),
            replay_status: "ok".to_string(),
            useful_change: false,
            already_promoted: false,
            risk: 0.10,
            target_file: "src/example.rs".to_string(),
        }
    }

    fn unsafe_class(run_id: &str) -> Self {
        let mut value = Self::ready(run_id);
        value.mutation_class = "unsafe".to_string();
        value
    }

    fn legacy(run_id: &str) -> Self {
        let mut value = Self::ready(run_id);
        value.mutation_class = "legacy".to_string();
        value
    }

    fn already_promoted(run_id: &str) -> Self {
        let mut value = Self::ready(run_id);
        value.already_promoted = true;
        value
    }

    fn needs_replay(run_id: &str) -> Self {
        let mut value = Self::ready(run_id);
        value.replay_status = "not_run".to_string();
        value
    }

    fn high_risk(run_id: &str) -> Self {
        let mut value = Self::ready(run_id);
        value.risk = 0.50;
        value
    }
}

fn temp_runtime_root(name: &str) -> PathBuf {
    let root = evolution_test_support::unique_evolution_root(name);
    fs::create_dir_all(root.join("src")).expect("src");
    fs::create_dir_all(root.join("tests")).expect("tests");
    fs::create_dir_all(root.join("memory")).expect("memory");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"phase80_temp\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\ndoctest = false\n",
    )
    .expect("cargo");
    fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("main");
    fs::write(root.join("src/lib.rs"), "pub fn probe() -> bool { true }\n").expect("lib");
    root
}

fn seed_candidate_fixture(root: &Path, fixture: CandidateFixture) {
    fs::create_dir_all(root.join("memory/candidates")).expect("candidates");
    fs::create_dir_all(root.join("memory/reports")).expect("reports");
    let digest = format!("digest-{}", fixture.run_id);
    let summary = CandidateSummary {
        run_id: fixture.run_id.clone(),
        mutation_id: format!("mutation-{}", fixture.run_id),
        mutation_digest: digest.clone(),
        status: EvolutionStatus::Candidate,
        target_file: fixture.target_file.clone(),
        mutation_kind: format!("{:?}", fixture.kind).to_ascii_lowercase(),
        risk: fixture.risk,
        score: 8.5,
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
        timestamp_unix: 100,
    };
    let fn_suffix = fixture.run_id.replace('-', "_");
    let mutation = MutationContract {
        id: summary.mutation_id.clone(),
        kind: fixture.kind,
        target_file: fixture.target_file.clone(),
        search: None,
        replace: None,
        append: Some(format!(
            "#[test]\nfn eva_generated_{}_deterministic() {{ assert!(true); }}\n",
            fn_suffix
        )),
        reason: "fixture".to_string(),
        expected_gain: 0.5,
        risk: fixture.risk,
    };
    let report = EvolutionReport {
        run_id: fixture.run_id.clone(),
        status: EvolutionStatus::Candidate,
        goal_ru: "fixture".to_string(),
        selected_plan_ru: "fixture".to_string(),
        mutation_ru: "fixture".to_string(),
        target_file: fixture.target_file.clone(),
        mutation_kind: summary.mutation_kind.clone(),
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
        replay_checked_at: Some(100),
        risk_ru: "ok".to_string(),
        next_step_ru: "ok".to_string(),
    };
    fs::write(
        root.join("memory/candidates")
            .join(format!("{}.summary.json", fixture.run_id)),
        serde_json::to_string_pretty(&summary).expect("summary"),
    )
    .expect("summary write");
    fs::write(
        root.join("memory/candidates")
            .join(format!("{}.mutation.json", fixture.run_id)),
        serde_json::to_string_pretty(&mutation).expect("mutation"),
    )
    .expect("mutation write");
    fs::write(
        root.join("memory/reports")
            .join(format!("{}.report.json", fixture.run_id)),
        serde_json::to_string_pretty(&report).expect("report"),
    )
    .expect("report write");
    fs::write(
        root.join("memory/reports")
            .join(format!("{}.ru.md", fixture.run_id)),
        "fixture report",
    )
    .expect("report md");
    if fixture.replay_status != "not_run" {
        fs::create_dir_all(root.join("memory/replays")).expect("replays");
        let replay = ReplayResult {
            run_id: fixture.run_id.clone(),
            replay_status: if fixture.replay_status == "ok" {
                EvolutionStatus::Candidate
            } else {
                EvolutionStatus::Failed
            },
            score: 8.5,
            matches_stored_summary: fixture.replay_status == "ok",
            cargo_check_ok: fixture.replay_status == "ok",
            cargo_test_ok: fixture.replay_status == "ok",
            cargo_run_ok: fixture.replay_status == "ok",
            stdout_digest: String::new(),
            stderr_digest: String::new(),
            stderr_tail: String::new(),
            sandbox_destroyed: true,
            timestamp_unix: 100,
        };
        fs::write(
            root.join("memory/replays")
                .join(format!("{}.json", fixture.run_id)),
            serde_json::to_string_pretty(&replay).expect("replay"),
        )
        .expect("replay write");
    }
    if fixture.already_promoted {
        append_evolution_log(
            &root.join("memory/evolution.jsonl"),
            &EvolutionLogEntry {
                run_id: format!("promoted-{}", fixture.run_id),
                plan_id: None,
                hypothesis_id: None,
                objective: None,
                graph_evidence: Vec::new(),
                recombined_source_patterns: Vec::new(),
                recombined_avoided_risks: Vec::new(),
                recombination_reason_ru: None,
                portfolio_reason_ru: None,
                selected_strategy: None,
                policy_reason_ru: None,
                mutation_class: "useful".to_string(),
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
                quality_score: 0.0,
                final_strategy_score: 0.0,
                mutation_id: format!("mutation-{}", fixture.run_id),
                mutation_digest: digest,
                status: EvolutionStatus::Promoted,
                target_file: fixture.target_file.clone(),
                mutation_kind: summary.mutation_kind.clone(),
                risk: fixture.risk,
                score: summary.score,
                useful_change: true,
                non_candidate_reason: None,
                duplicate_rejected: false,
                regression_penalty: 0.0,
                success_bonus: 0.0,
                cargo_check_ok: true,
                cargo_test_ok: true,
                cargo_run_ok: true,
                retained_in_core: true,
                sandbox_destroyed: true,
                stdout_digest: String::new(),
                stderr_digest: String::new(),
                stderr_tail: String::new(),
                timestamp_unix: 101,
            },
        );
    }
}

fn mutate_candidate_score(root: &Path, run_id: &str, score: f32) {
    let path = root
        .join("memory/candidates")
        .join(format!("{run_id}.summary.json"));
    let mut summary: CandidateSummary =
        serde_json::from_str(&fs::read_to_string(&path).expect("read summary")).expect("parse");
    summary.score = score;
    fs::write(
        path,
        serde_json::to_string_pretty(&summary).expect("summary"),
    )
    .expect("write");
}

fn mutate_candidate_target(root: &Path, run_id: &str, target: &str) {
    let summary_path = root
        .join("memory/candidates")
        .join(format!("{run_id}.summary.json"));
    let mutation_path = root
        .join("memory/candidates")
        .join(format!("{run_id}.mutation.json"));
    let report_path = root
        .join("memory/reports")
        .join(format!("{run_id}.report.json"));
    let mut summary: CandidateSummary =
        serde_json::from_str(&fs::read_to_string(&summary_path).expect("read summary"))
            .expect("parse");
    let mut mutation: MutationContract =
        serde_json::from_str(&fs::read_to_string(&mutation_path).expect("read mutation"))
            .expect("parse");
    let mut report: EvolutionReport =
        serde_json::from_str(&fs::read_to_string(&report_path).expect("read report"))
            .expect("parse");
    summary.target_file = target.to_string();
    mutation.target_file = target.to_string();
    report.target_file = target.to_string();
    fs::write(
        summary_path,
        serde_json::to_string_pretty(&summary).expect("summary"),
    )
    .expect("write summary");
    fs::write(
        mutation_path,
        serde_json::to_string_pretty(&mutation).expect("mutation"),
    )
    .expect("write mutation");
    fs::write(
        report_path,
        serde_json::to_string_pretty(&report).expect("report"),
    )
    .expect("write report");
}

fn seed_autonomy_memory(root: &Path) {
    fs::create_dir_all(root.join("memory/replays")).expect("replays");
    fs::write(root.join("memory/regressions.json"), "[]").expect("regressions");
    fs::write(root.join("memory/success_patterns.json"), "[]").expect("success");
    let mut lines = Vec::new();
    for index in 0..12 {
        lines.push(format!(
            "{{\"run_id\":\"seed-{index}\",\"plan_id\":null,\"hypothesis_id\":null,\"objective\":\"ImproveTests\",\"graph_evidence\":[],\"recombined_source_patterns\":[],\"recombined_avoided_risks\":[],\"recombination_reason_ru\":null,\"portfolio_reason_ru\":null,\"selected_strategy\":null,\"policy_reason_ru\":null,\"mutation_class\":\"useful\",\"hygiene_warning_ru\":null,\"diversity_bonus\":0.0,\"saturation_penalty\":0.0,\"repeated_target_penalty\":0.0,\"final_recombination_score\":0.0,\"strategy_bonus\":0.0,\"strategy_saturation_penalty\":0.0,\"quality_bonus\":0.0,\"novelty_score\":0.0,\"useful_delta_score\":0.0,\"duplicate_suppression_score\":0.0,\"regression_avoidance_score\":0.0,\"coverage_proxy_score\":0.0,\"quality_score\":0.0,\"final_strategy_score\":0.0,\"mutation_id\":\"m-{index}\",\"mutation_digest\":\"d-{index}\",\"status\":\"candidate\",\"target_file\":\"tests/evolution_generated_tests.rs\",\"mutation_kind\":\"addunittest\",\"risk\":0.10,\"score\":8.50,\"useful_change\":true,\"non_candidate_reason\":null,\"duplicate_rejected\":false,\"regression_penalty\":0.0,\"success_bonus\":0.0,\"cargo_check_ok\":true,\"cargo_test_ok\":true,\"cargo_run_ok\":true,\"retained_in_core\":false,\"sandbox_destroyed\":true,\"stdout_digest\":\"\",\"stderr_digest\":\"\",\"stderr_tail\":\"\",\"timestamp_unix\":{}}}",
            index + 1
        ));
    }
    fs::write(root.join("memory/evolution.jsonl"), lines.join("\n") + "\n").expect("evolution");
    fs::write(
        root.join("memory/graph.json"),
        serde_json::to_string_pretty(&EvolutionGraph {
            nodes: vec![GraphNode {
                id: "file:tests/evolution_generated_tests.rs".to_string(),
                kind: "File".to_string(),
            }],
            edges: vec![GraphEdge {
                from: "pattern:test".to_string(),
                to: "file:tests/evolution_generated_tests.rs".to_string(),
                relation: "supports".to_string(),
            }],
        })
        .expect("graph"),
    )
    .expect("graph");
    fs::write(
        root.join("memory/metrics.json"),
        r#"{"total_runs":12,"passed_runs":12,"failed_runs":0,"candidate_count":12,"replay_passed":4,"promoted_count":0,"average_score":8.5,"last_run_id":"seed-11"}"#,
    )
    .expect("metrics");
}

fn append_evolution_log(path: &Path, entry: &EvolutionLogEntry) {
    let mut lines = if path.exists() {
        fs::read_to_string(path).expect("read log")
    } else {
        String::new()
    };
    lines.push_str(&serde_json::to_string(entry).expect("entry"));
    lines.push('\n');
    fs::write(path, lines).expect("write log");
}
