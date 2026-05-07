use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "evolution_test_support.rs"]
mod evolution_test_support;

use eva_runtime_with_task_validator::contracts::EvolutionStatus;
use eva_runtime_with_task_validator::evolution::{CandidateSummary, ReplayResult};
use eva_runtime_with_task_validator::graph::{GraphEdge, GraphNode};
use eva_runtime_with_task_validator::{
    approve_candidate, build_release_bundle, build_release_preflight, latest_release_id,
    list_releases, print_last_release, print_proof_json, print_proof_report,
    print_release_manifest, print_release_status, print_rollback_manifest, EvolutionGraph,
    EvolutionReport, MutationContract, MutationKind, ReleaseManifest, ReleasePreflightReport,
    RollbackManifest,
};
use serde_json::Value;

#[test]
fn release_preflight_rejects_cosmetic_candidate() {
    let root = temp_runtime_root("phase90-cosmetic");
    seed_autonomy_memory(&root);
    seed_release_candidate(
        &root,
        ReleaseCandidateFixture::cosmetic("cosmetic-release", 2_000_000_000),
    );
    let memory_root = root.join("memory");

    let report = build_release_preflight(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "cosmetic-release",
    )
    .expect("preflight");
    assert!(!report.allowed);
    assert!(report
        .blockers
        .contains(&"appendcomment_cosmetic".to_string()));
    assert!(report.blockers.contains(&"cosmetic_mutation".to_string()));
    assert!(!root.join("sandboxes").exists());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn release_preflight_rejects_unapproved_candidate() {
    let root = temp_runtime_root("phase90-unapproved");
    seed_autonomy_memory(&root);
    seed_release_candidate(
        &root,
        ReleaseCandidateFixture::useful("unapproved-release", 2_000_000_010),
    );
    let memory_root = root.join("memory");

    let report = build_release_preflight(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "unapproved-release",
    )
    .expect("preflight");
    assert!(!report.allowed);
    assert!(report.blockers.contains(&"approval_required".to_string()));
    assert!(report.blockers.contains(&"not_approved".to_string()));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn release_preflight_accepts_governance_approved_replay_ok_candidate() {
    let root = temp_runtime_root("phase90-approved");
    seed_autonomy_memory(&root);
    seed_release_candidate(
        &root,
        ReleaseCandidateFixture::useful("approved-release", 2_000_000_100),
    );
    let memory_root = root.join("memory");
    approve_candidate(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "approved-release",
        "safe release",
    )
    .expect("approve");

    let report = build_release_preflight(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "approved-release",
    )
    .expect("preflight");
    assert!(report.allowed);
    assert!(report.approved);
    assert_eq!(report.replay_status, "ok");
    assert_eq!(report.mutation_class, "useful");

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn release_bundle_writes_manifest_changelog_and_rollback() {
    let root = temp_runtime_root("phase90-bundle");
    seed_autonomy_memory(&root);
    seed_release_candidate(
        &root,
        ReleaseCandidateFixture::useful("bundle-release", 2_000_000_200),
    );
    let memory_root = root.join("memory");
    approve_candidate(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "bundle-release",
        "ship it",
    )
    .expect("approve");

    let bundle = build_release_bundle(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "bundle-release",
    )
    .expect("bundle");
    assert_eq!(bundle.approval_status, "approve");
    assert!(!bundle.safety_notes.is_empty());
    assert!(Path::new(&bundle.release_manifest_path).exists());
    assert!(Path::new(&bundle.rollback_manifest_path).exists());
    assert!(Path::new(&bundle.changelog_path).exists());
    assert!(Path::new(&bundle.preflight_report_path).exists());
    assert!(Path::new(&bundle.candidate_report_path.clone().unwrap()).exists());

    let manifest: ReleaseManifest =
        serde_json::from_str(&fs::read_to_string(&bundle.release_manifest_path).expect("manifest"))
            .expect("parse manifest");
    let rollback: RollbackManifest = serde_json::from_str(
        &fs::read_to_string(&bundle.rollback_manifest_path).expect("rollback"),
    )
    .expect("parse rollback");
    let preflight: ReleasePreflightReport = serde_json::from_str(
        &fs::read_to_string(&bundle.preflight_report_path).expect("preflight"),
    )
    .expect("parse preflight");
    assert!(manifest.approved);
    assert!(manifest.rollback_available);
    assert!(manifest.changelog_available);
    assert!(rollback.rollback_available);
    assert!(preflight.allowed);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn release_bundle_is_deterministic() {
    let root = temp_runtime_root("phase90-deterministic");
    seed_autonomy_memory(&root);
    seed_release_candidate(
        &root,
        ReleaseCandidateFixture::useful("deterministic-release", 2_000_000_300),
    );
    let memory_root = root.join("memory");
    approve_candidate(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "deterministic-release",
        "ship it",
    )
    .expect("approve");

    let first = build_release_bundle(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "deterministic-release",
    )
    .expect("bundle first");
    let second = build_release_bundle(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "deterministic-release",
    )
    .expect("bundle second");
    assert_eq!(first, second);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn release_bundle_does_not_mutate_source_files() {
    let root = temp_runtime_root("phase90-source");
    seed_autonomy_memory(&root);
    seed_release_candidate(
        &root,
        ReleaseCandidateFixture::useful("source-release", 2_000_000_400),
    );
    let memory_root = root.join("memory");
    approve_candidate(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "source-release",
        "ship it",
    )
    .expect("approve");

    let before_main = fs::read_to_string(root.join("src/main.rs")).expect("main before");
    let before_lib = fs::read_to_string(root.join("src/lib.rs")).expect("lib before");
    let before_target =
        fs::read_to_string(root.join("tests/evolution_generated_tests.rs")).expect("target before");
    let _ = build_release_bundle(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "source-release",
    )
    .expect("bundle");

    assert_eq!(
        before_main,
        fs::read_to_string(root.join("src/main.rs")).expect("main after")
    );
    assert_eq!(
        before_lib,
        fs::read_to_string(root.join("src/lib.rs")).expect("lib after")
    );
    assert_eq!(
        before_target,
        fs::read_to_string(root.join("tests/evolution_generated_tests.rs")).expect("target after")
    );
    assert!(!root.join("sandboxes").exists());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn release_bundle_never_auto_promotes() {
    let root = temp_runtime_root("phase90-auto-promote");
    seed_autonomy_memory(&root);
    seed_release_candidate(
        &root,
        ReleaseCandidateFixture::useful("auto-release", 2_000_000_500),
    );
    let memory_root = root.join("memory");
    approve_candidate(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "auto-release",
        "ship it",
    )
    .expect("approve");

    let bundle = build_release_bundle(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "auto-release",
    )
    .expect("bundle");
    assert!(bundle
        .safety_notes
        .iter()
        .any(|note| note == "auto_promote=false"));
    let manifest: ReleaseManifest =
        serde_json::from_str(&fs::read_to_string(&bundle.release_manifest_path).expect("manifest"))
            .expect("parse manifest");
    assert!(!manifest.auto_promote);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn release_manifest_prints_existing_release() {
    let root = temp_runtime_root("phase90-manifest");
    seed_autonomy_memory(&root);
    seed_release_candidate(
        &root,
        ReleaseCandidateFixture::useful("manifest-release", 2_000_000_600),
    );
    let memory_root = root.join("memory");
    approve_candidate(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "manifest-release",
        "ship it",
    )
    .expect("approve");

    let bundle = build_release_bundle(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "manifest-release",
    )
    .expect("bundle");
    let manifest_json = print_release_manifest(memory_root.to_str().unwrap(), &bundle.release_id)
        .expect("manifest");
    let manifest: ReleaseManifest = serde_json::from_str(&manifest_json).expect("parse");
    assert_eq!(manifest.release_id, bundle.release_id);
    assert_eq!(manifest.source_run_id, "manifest-release");

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn list_releases_is_deterministic() {
    let root = temp_runtime_root("phase90-list");
    seed_autonomy_memory(&root);
    seed_release_candidate(
        &root,
        ReleaseCandidateFixture::useful("release-old", 2_000_000_700),
    );
    seed_release_candidate(
        &root,
        ReleaseCandidateFixture::useful("release-new", 2_000_000_800),
    );
    let memory_root = root.join("memory");
    approve_candidate(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "release-old",
        "ship it",
    )
    .expect("approve old");
    approve_candidate(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "release-new",
        "ship it",
    )
    .expect("approve new");
    let _ = build_release_bundle(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "release-old",
    )
    .expect("bundle old");
    let _ = build_release_bundle(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "release-new",
    )
    .expect("bundle new");

    let first = list_releases(memory_root.to_str().unwrap()).expect("list first");
    let second = list_releases(memory_root.to_str().unwrap()).expect("list second");
    assert_eq!(first, second);
    assert_eq!(
        latest_release_id(memory_root.to_str().unwrap()).expect("latest"),
        second.last().cloned()
    );

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn last_release_selects_newest_release() {
    let root = temp_runtime_root("phase90-last");
    seed_autonomy_memory(&root);
    seed_release_candidate(
        &root,
        ReleaseCandidateFixture::useful("last-old", 2_000_000_900),
    );
    seed_release_candidate(
        &root,
        ReleaseCandidateFixture::useful("last-new", 2_000_001_000),
    );
    let memory_root = root.join("memory");
    approve_candidate(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "last-old",
        "ship it",
    )
    .expect("approve old");
    approve_candidate(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "last-new",
        "ship it",
    )
    .expect("approve new");
    let _ = build_release_bundle(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "last-old",
    )
    .expect("bundle old");
    let _ = build_release_bundle(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "last-new",
    )
    .expect("bundle new");

    let report = print_last_release(memory_root.to_str().unwrap()).expect("last release");
    assert!(report.contains("last-new"));
    assert!(!report.contains("last-old\n"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn rollback_manifest_preserves_target_and_original_candidate() {
    let root = temp_runtime_root("phase90-rollback");
    seed_autonomy_memory(&root);
    seed_release_candidate(
        &root,
        ReleaseCandidateFixture::useful("rollback-release", 2_000_001_100),
    );
    let memory_root = root.join("memory");
    approve_candidate(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "rollback-release",
        "ship it",
    )
    .expect("approve");

    let bundle = build_release_bundle(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "rollback-release",
    )
    .expect("bundle");
    let json = print_rollback_manifest(memory_root.to_str().unwrap(), &bundle.release_id)
        .expect("rollback");
    let rollback: RollbackManifest = serde_json::from_str(&json).expect("parse rollback");
    assert_eq!(rollback.target_file, "tests/evolution_generated_tests.rs");
    assert_eq!(rollback.source_run_id, "rollback-release");
    assert!(rollback.original_candidate_report_path.is_some());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn release_status_reports_auto_promote_false() {
    let root = temp_runtime_root("phase90-status");
    seed_autonomy_memory(&root);
    seed_release_candidate(
        &root,
        ReleaseCandidateFixture::useful("status-release", 2_000_001_200),
    );
    let memory_root = root.join("memory");
    approve_candidate(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "status-release",
        "ship it",
    )
    .expect("approve");
    let _ = build_release_bundle(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "status-release",
    )
    .expect("bundle");

    let status = print_release_status(memory_root.to_str().unwrap()).expect("status");
    assert!(status.contains("auto_promote=false"));
    assert!(status.contains("approval_required=true"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn proof_report_includes_release_runtime_support() {
    let root = temp_runtime_root("phase90-proof-report");
    seed_autonomy_memory(&root);
    seed_release_candidate(
        &root,
        ReleaseCandidateFixture::useful("proof-release", 2_000_001_300),
    );
    let memory_root = root.join("memory");
    approve_candidate(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "proof-release",
        "ship it",
    )
    .expect("approve");
    let _ = build_release_bundle(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "proof-release",
    )
    .expect("bundle");

    let report = print_proof_report(root.to_str().unwrap(), memory_root.to_str().unwrap())
        .expect("proof report");
    assert!(report.contains("release_runtime_support=true"));
    assert!(report.contains("release_count=1"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn proof_json_includes_release_count_and_latest_release_id() {
    let root = temp_runtime_root("phase90-proof-json");
    seed_autonomy_memory(&root);
    seed_release_candidate(
        &root,
        ReleaseCandidateFixture::useful("proof-json-release", 2_000_001_400),
    );
    let memory_root = root.join("memory");
    approve_candidate(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "proof-json-release",
        "ship it",
    )
    .expect("approve");
    let bundle = build_release_bundle(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "proof-json-release",
    )
    .expect("bundle");

    let json = print_proof_json(root.to_str().unwrap(), memory_root.to_str().unwrap())
        .expect("proof json");
    let value: Value = serde_json::from_str(&json).expect("parse proof json");
    assert_eq!(value["release_count"].as_u64(), Some(1));
    assert_eq!(
        value["latest_release_id"].as_str(),
        Some(bundle.release_id.as_str())
    );
    assert_eq!(value["release_runtime_support"].as_bool(), Some(true));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn appendcomment_never_becomes_release() {
    let root = temp_runtime_root("phase90-appendcomment");
    seed_autonomy_memory(&root);
    seed_release_candidate(
        &root,
        ReleaseCandidateFixture::cosmetic("appendcomment-release", 2_000_001_500),
    );
    let memory_root = root.join("memory");

    let preflight = build_release_preflight(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "appendcomment-release",
    )
    .expect("preflight");
    assert!(!preflight.allowed);
    assert!(preflight
        .blockers
        .contains(&"appendcomment_cosmetic".to_string()));
    assert!(build_release_bundle(
        root.to_str().unwrap(),
        memory_root.to_str().unwrap(),
        "appendcomment-release",
    )
    .is_err());

    fs::remove_dir_all(root).expect("cleanup");
}

#[derive(Clone)]
struct ReleaseCandidateFixture {
    run_id: String,
    kind: MutationKind,
    mutation_class: String,
    replay_status: String,
    useful_change: bool,
    risk: f32,
    target_file: String,
    score: f32,
    timestamp_unix: u64,
}

impl ReleaseCandidateFixture {
    fn useful(run_id: &str, timestamp_unix: u64) -> Self {
        Self {
            run_id: run_id.to_string(),
            kind: MutationKind::AddUnitTest,
            mutation_class: "useful".to_string(),
            replay_status: "ok".to_string(),
            useful_change: true,
            risk: 0.10,
            target_file: "tests/evolution_generated_tests.rs".to_string(),
            score: 9.5,
            timestamp_unix,
        }
    }

    fn cosmetic(run_id: &str, timestamp_unix: u64) -> Self {
        Self {
            run_id: run_id.to_string(),
            kind: MutationKind::AppendComment,
            mutation_class: "cosmetic".to_string(),
            replay_status: "ok".to_string(),
            useful_change: false,
            risk: 0.10,
            target_file: "src/runtime_cycle.rs".to_string(),
            score: 9.5,
            timestamp_unix,
        }
    }
}

fn temp_runtime_root(name: &str) -> PathBuf {
    let root = evolution_test_support::unique_evolution_root(name);
    fs::create_dir_all(root.join("src/evolution")).expect("src");
    fs::create_dir_all(root.join("tests")).expect("tests");
    fs::create_dir_all(root.join("memory")).expect("memory");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"phase90_temp\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\ndoctest = false\n",
    )
    .expect("cargo");
    fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("main");
    fs::write(root.join("src/lib.rs"), "pub fn probe() -> bool { true }\n").expect("lib");
    fs::write(
        root.join("src/evolution/metrics.rs"),
        "pub fn metric() -> u64 { 1 }\n",
    )
    .expect("metrics");
    fs::write(
        root.join("src/runtime_cycle.rs"),
        "pub fn runtime_cycle() {}\n",
    )
    .expect("runtime cycle");
    fs::write(
        root.join("tests/evolution_generated_tests.rs"),
        "#[test]\nfn existing_test() { assert!(true); }\n",
    )
    .expect("generated tests");
    root
}

fn seed_release_candidate(root: &Path, fixture: ReleaseCandidateFixture) {
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
        score: fixture.score,
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
    let fn_suffix = fixture.run_id.replace('-', "_");
    let mutation = MutationContract {
        id: summary.mutation_id.clone(),
        kind: fixture.kind,
        target_file: fixture.target_file.clone(),
        search: None,
        replace: None,
        append: Some(match fixture.kind {
            MutationKind::AppendComment => "// EVA release metadata-only fixture.".to_string(),
            _ => format!(
                "#[test]\nfn eva_generated_{}_deterministic() {{ assert!(true); }}\n",
                fn_suffix
            ),
        }),
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
        replay_checked_at: Some(fixture.timestamp_unix),
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
            score: fixture.score,
            matches_stored_summary: fixture.replay_status == "ok",
            cargo_check_ok: fixture.replay_status == "ok",
            cargo_test_ok: fixture.replay_status == "ok",
            cargo_run_ok: fixture.replay_status == "ok",
            stdout_digest: String::new(),
            stderr_digest: String::new(),
            stderr_tail: String::new(),
            sandbox_destroyed: true,
            timestamp_unix: fixture.timestamp_unix,
        };
        fs::write(
            root.join("memory/replays")
                .join(format!("{}.json", fixture.run_id)),
            serde_json::to_string_pretty(&replay).expect("replay"),
        )
        .expect("replay write");
    }
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
        .expect("replay write");
    }
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
