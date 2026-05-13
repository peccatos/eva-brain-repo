use std::fs;
use std::path::{Path, PathBuf};

#[path = "evolution_test_support.rs"]
mod evolution_test_support;

use eva_runtime_with_task_validator::evolution::{CandidateSummary, ReplayResult};
use eva_runtime_with_task_validator::{
    approve_release_candidate, build_runtime_validation, load_tui_state_from_project_root,
    refresh_promotion_queue, render_tui_snapshot, CandidateState, EvolutionLogEntry,
    EvolutionMetrics, EvolutionStatus, PromotionQueue, PromotionQueueItem, RuntimeValidation,
};

#[test]
fn tui_loads_metrics_json_and_latest_run_from_real_memory() {
    let root = temp_root("phase151h-tui-metrics");
    write_metrics(
        &root,
        EvolutionMetrics {
            total_runs: 34,
            passed_runs: 21,
            failed_runs: 4,
            safety_rejected_runs: 5,
            duplicate_rejected_runs: 5,
            candidate_count: 15,
            replay_passed: 10,
            replay_failed: 2,
            promoted_count: 6,
            last_run_id: Some("run-1778235862201-556497".to_string()),
            ..EvolutionMetrics::default()
        },
    );
    write_queue(&root, sample_queue());
    write_validation(
        &root,
        RuntimeValidation {
            status: "warn".to_string(),
            warnings: vec![
                "preflight_gate_v3_warn".to_string(),
                "release_health_yellow".to_string(),
                "no_approved_release_candidate".to_string(),
            ],
            missing_green_conditions: vec![
                "operator_approved".to_string(),
                "release_bundle_exists".to_string(),
            ],
            blockers: Vec::new(),
            ..RuntimeValidation::default()
        },
    );
    write_replay(
        &root,
        "run-1778235862201-556497",
        EvolutionStatus::Candidate,
        true,
        true,
        true,
    );

    let state = load_tui_state_from_project_root(&root);

    assert_eq!(
        state.dashboard.latest_run_id.as_deref(),
        Some("run-1778235862201-556497")
    );
    assert_eq!(state.dashboard.candidate_count, 15);
    assert_eq!(state.metrics.total_runs, 34);
    assert_eq!(state.metrics.replay_passed, 10);
    assert_eq!(state.metrics.replay_failed, 2);
    assert_eq!(state.dashboard.last_replay_status, "passed");
    evolution_test_support::remove_root(&root);
}

#[test]
fn tui_uses_promotion_queue_count_when_metrics_candidate_count_missing() {
    let root = temp_root("phase151h-tui-queue-count");
    write_metrics(
        &root,
        EvolutionMetrics {
            total_runs: 1,
            last_run_id: Some("queue-run-1".to_string()),
            ..EvolutionMetrics::default()
        },
    );
    let mut queue = sample_queue();
    queue.summary.candidate_count = 2;
    queue.items.truncate(2);
    write_queue(&root, queue);

    let state = load_tui_state_from_project_root(&root);

    assert_eq!(state.dashboard.candidate_count, 2);
    assert_eq!(state.candidates.len(), 2);
    evolution_test_support::remove_root(&root);
}

#[test]
fn tui_separates_warnings_from_missing_green_conditions() {
    let root = temp_root("phase151h-tui-warnings");
    write_metrics(&root, EvolutionMetrics::default());
    write_queue(&root, PromotionQueue::default());
    write_validation(
        &root,
        RuntimeValidation {
            status: "warn".to_string(),
            warnings: vec![
                "preflight_gate_v3_warn".to_string(),
                "release_health_yellow".to_string(),
                "no_approved_release_candidate".to_string(),
            ],
            missing_green_conditions: vec![
                "operator_approved".to_string(),
                "preflight_gate_v3_pass".to_string(),
                "release_bundle_exists".to_string(),
                "release_health_green".to_string(),
            ],
            blockers: Vec::new(),
            ..RuntimeValidation::default()
        },
    );

    let state = load_tui_state_from_project_root(&root);
    let snapshot = render_tui_snapshot(&state);

    assert_eq!(
        state.dashboard.warnings,
        vec![
            "no_approved_release_candidate",
            "preflight_gate_v3_warn",
            "release_health_yellow",
        ]
    );
    assert_eq!(
        state.dashboard.missing_green_conditions,
        vec![
            "operator_approved",
            "preflight_gate_v3_pass",
            "release_bundle_exists",
            "release_health_green",
        ]
    );
    assert!(snapshot.contains(
        "warnings=no_approved_release_candidate,preflight_gate_v3_warn,release_health_yellow"
    ));
    assert!(snapshot.contains(
        "missing_green_conditions=operator_approved,preflight_gate_v3_pass,release_bundle_exists,release_health_green"
    ));
    evolution_test_support::remove_root(&root);
}

#[test]
fn tui_missing_files_do_not_panic_and_tests_stay_non_interactive() {
    let root = temp_root("phase151h-tui-missing");
    let state = load_tui_state_from_project_root(&root);
    let snapshot = render_tui_snapshot(&state);

    assert_eq!(state.dashboard.latest_run_id, None);
    assert_eq!(state.dashboard.last_replay_status, "missing");
    assert!(state.runs.is_empty());
    assert!(state.candidates.is_empty());
    assert!(snapshot.contains("EVA Operator TUI"));
    assert!(snapshot.contains("Dashboard"));
    evolution_test_support::remove_root(&root);
}

#[test]
fn tui_parse_errors_are_visible() {
    let root = temp_root("phase151h-tui-parse-errors");
    fs::write(root.join("memory/metrics.json"), "{not-json").expect("bad metrics");

    let state = load_tui_state_from_project_root(&root);

    assert!(
        state
            .parse_messages
            .iter()
            .any(|message| message.contains("parse_error:")
                && message.contains("memory/metrics.json")),
        "{:?}",
        state.parse_messages
    );
    evolution_test_support::remove_root(&root);
}

#[test]
fn release_approval_refuses_quarantined_candidate() {
    let root = temp_root("phase151h-release-quarantined");
    seed_candidate(
        &root,
        "quarantine-run",
        "tests/evolution_generated_tests.rs",
        EvolutionStatus::Candidate,
        true,
        false,
        false,
    );
    refresh_promotion_queue(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("queue");

    let err = approve_release_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "quarantine-run",
    )
    .expect_err("quarantined candidate must fail");

    assert!(err.contains("release approval refused"));
    assert!(err.contains("reason=candidate_state=quarantined"));
    assert!(err.contains("cargo_run_not_ok") || err.contains("cargo_test_not_ok"));
    evolution_test_support::remove_root(&root);
}

#[test]
fn release_approval_refuses_blocked_candidate() {
    let root = temp_root("phase151h-release-blocked");
    seed_candidate(
        &root,
        "blocked-run",
        "src/core/forbidden.rs",
        EvolutionStatus::Candidate,
        true,
        true,
        false,
    );
    refresh_promotion_queue(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("queue");

    let err = approve_release_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "blocked-run",
    )
    .expect_err("blocked candidate must fail");

    assert!(err.contains("release approval refused"));
    assert!(err.contains("reason=candidate_state=blocked"));
    assert!(err.contains("blockers="));
    evolution_test_support::remove_root(&root);
}

#[test]
fn release_approval_refuses_failed_replay_candidate() {
    let root = temp_root("phase151h-release-failed-replay");
    seed_candidate(
        &root,
        "failed-replay-run",
        "tests/evolution_generated_tests.rs",
        EvolutionStatus::Failed,
        true,
        true,
        false,
    );
    refresh_promotion_queue(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("queue");

    let err = approve_release_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "failed-replay-run",
    )
    .expect_err("failed replay candidate must fail");

    assert!(err.contains("release approval refused"));
    assert!(
        err.contains("reason=candidate_state=unreplayable")
            || err.contains("reason=replay_status=failed")
    );
    assert!(err.contains("replay_not_ok"));
    evolution_test_support::remove_root(&root);
}

#[test]
fn runtime_green_is_impossible_without_approved_rc_or_release_bundle() {
    let root = temp_root("phase151h-green-impossible");

    let validation = build_runtime_validation(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("validation");

    assert_eq!(validation.status, "warn");
    assert!(validation
        .missing_green_conditions
        .contains(&"approved_release_candidate".to_string()));
    assert!(validation
        .missing_green_conditions
        .contains(&"release_bundle_exists".to_string()));
    assert_ne!(validation.status, "green");
    evolution_test_support::remove_root(&root);
}

fn temp_root(name: &str) -> PathBuf {
    let root = evolution_test_support::unique_evolution_root(name);
    fs::create_dir_all(root.join("src")).expect("src");
    fs::create_dir_all(root.join("tests")).expect("tests");
    fs::create_dir_all(root.join("memory")).expect("memory");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname=\"phase151_temp\"\nversion=\"0.1.0\"\nedition=\"2021\"\n\n[lib]\ndoctest=false\n",
    )
    .expect("cargo");
    fs::write(root.join("src/lib.rs"), "pub fn probe() -> bool { true }\n").expect("lib");
    fs::write(root.join("memory/regressions.json"), "[]").expect("regressions");
    fs::write(root.join("memory/success_patterns.json"), "[]").expect("success");
    root
}

fn write_metrics(root: &Path, metrics: EvolutionMetrics) {
    fs::create_dir_all(root.join("memory")).expect("memory");
    fs::write(
        root.join("memory/metrics.json"),
        serde_json::to_string_pretty(&metrics).expect("metrics json"),
    )
    .expect("write metrics");
}

fn write_queue(root: &Path, queue: PromotionQueue) {
    fs::create_dir_all(root.join("memory")).expect("memory");
    fs::write(
        root.join("memory/promotion_queue.json"),
        serde_json::to_string_pretty(&queue).expect("queue json"),
    )
    .expect("write queue");
}

fn write_validation(root: &Path, validation: RuntimeValidation) {
    let dir = root.join("memory/runtime_validation");
    fs::create_dir_all(&dir).expect("runtime validation dir");
    fs::write(
        dir.join("runtime-validation-test.json"),
        serde_json::to_string_pretty(&validation).expect("validation json"),
    )
    .expect("write validation");
}

fn write_replay(
    root: &Path,
    run_id: &str,
    replay_status: EvolutionStatus,
    matches_stored_summary: bool,
    cargo_test_ok: bool,
    cargo_run_ok: bool,
) {
    let dir = root.join("memory/replays");
    fs::create_dir_all(&dir).expect("replays");
    let replay = ReplayResult {
        run_id: run_id.to_string(),
        replay_status,
        score: 8.0,
        matches_stored_summary,
        cargo_check_ok: cargo_test_ok,
        cargo_test_ok,
        cargo_run_ok,
        stdout_digest: String::new(),
        stderr_digest: String::new(),
        stderr_tail: String::new(),
        sandbox_destroyed: true,
        timestamp_unix: 1,
    };
    fs::write(
        dir.join(format!("{run_id}.json")),
        serde_json::to_string_pretty(&replay).expect("replay json"),
    )
    .expect("write replay");
}

fn sample_queue() -> PromotionQueue {
    PromotionQueue {
        generated_at: 1,
        summary: eva_runtime_with_task_validator::CandidateQueueSummary {
            candidate_count: 3,
            ready_candidates: 1,
            blocked_candidates: 1,
            quarantined_candidates: 1,
            legacy_candidates: 0,
            duplicate_candidates: 0,
            unreplayable_candidates: 1,
            already_promoted_candidates: 0,
            unknown_candidates: 0,
        },
        items: vec![
            queue_item(
                "queue-run-1",
                CandidateState::Ready,
                "ok",
                true,
                true,
                false,
            ),
            queue_item(
                "queue-run-2",
                CandidateState::Quarantined,
                "failed",
                true,
                false,
                false,
            ),
            queue_item(
                "queue-run-3",
                CandidateState::Unreplayable,
                "failed",
                true,
                true,
                false,
            ),
        ],
    }
}

fn queue_item(
    run_id: &str,
    state: CandidateState,
    replay_status: &str,
    cargo_test_ok: bool,
    cargo_run_ok: bool,
    duplicate: bool,
) -> PromotionQueueItem {
    PromotionQueueItem {
        run_id: run_id.to_string(),
        mutation_kind: "add_unit_test".to_string(),
        mutation_class: "useful".to_string(),
        target_file: "tests/evolution_generated_tests.rs".to_string(),
        score: 8.0,
        risk: 0.1,
        replay_status: replay_status.to_string(),
        promotion_state: if matches!(state, CandidateState::Ready) {
            "ready".to_string()
        } else {
            "blocked".to_string()
        },
        promotion_allowed: matches!(state, CandidateState::Ready),
        promotion_blockers: Vec::new(),
        report_path: format!("memory/reports/{run_id}.report.json"),
        lifecycle_state: if matches!(state, CandidateState::Ready) {
            "ready".to_string()
        } else {
            "blocked".to_string()
        },
        candidate_state: state.clone(),
        candidate_state_reason: format!("{:?}", state),
        cargo_test_ok: Some(cargo_test_ok),
        cargo_run_ok: Some(cargo_run_ok),
        duplicate_rejected: duplicate,
        promoted: false,
        reason_ru: String::new(),
        updated_at: 1,
    }
}

fn seed_candidate(
    root: &Path,
    run_id: &str,
    target_file: &str,
    replay_status: EvolutionStatus,
    cargo_test_ok: bool,
    cargo_run_ok: bool,
    duplicate: bool,
) {
    fs::create_dir_all(root.join("memory/candidates")).expect("candidates");
    fs::create_dir_all(root.join("memory/reports")).expect("reports");
    let entry = log_entry(run_id, target_file, cargo_test_ok, cargo_run_ok, duplicate);
    fs::write(
        root.join("memory/evolution.jsonl"),
        format!("{}\n", serde_json::to_string(&entry).expect("log")),
    )
    .expect("write log");
    fs::write(
        root.join("memory/candidates")
            .join(format!("{run_id}.summary.json")),
        serde_json::to_string_pretty(&CandidateSummary::from(&entry)).expect("summary"),
    )
    .expect("summary file");
    fs::write(
        root.join("memory/candidates").join(format!("{run_id}.mutation.json")),
        r##"{"id":"m","kind":"add_unit_test","target_file":"tests/evolution_generated_tests.rs","search":null,"replace":null,"append":"#[test]\nfn eva_generated_probe() { assert!(true); }\n","reason":"test fixture","expected_gain":0.5,"risk":0.1}"##,
    )
    .expect("mutation");
    fs::write(
        root.join("memory/reports").join(format!("{run_id}.ru.md")),
        "report",
    )
    .expect("report md");
    fs::write(
        root.join("memory/reports").join(format!("{run_id}.report.json")),
        format!(
            r#"{{"run_id":"{run_id}","status":"candidate","goal_ru":"","selected_plan_ru":"","mutation_ru":"","target_file":"{target_file}","mutation_kind":"add_unit_test","mutation_class":"useful","sandbox_ru":"","checks_ru":"","score_ru":"","candidate_ru":"","replay_ru":"","replay_status":"{}","risk_ru":"","next_step_ru":"","quality_score":0.9,"novelty_score":0.9,"useful_delta_score":0.9,"regression_avoidance_score":0.9}}"#,
            if matches!(replay_status, EvolutionStatus::Failed) {
                "failed"
            } else {
                "ok"
            }
        ),
    )
    .expect("report json");
    write_replay(
        root,
        run_id,
        replay_status,
        !matches!(replay_status, EvolutionStatus::Failed),
        cargo_test_ok,
        cargo_run_ok,
    );
}

fn log_entry(
    run_id: &str,
    target_file: &str,
    cargo_test_ok: bool,
    cargo_run_ok: bool,
    duplicate: bool,
) -> EvolutionLogEntry {
    EvolutionLogEntry {
        run_id: run_id.to_string(),
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
        quality_score: 0.9,
        final_strategy_score: 0.9,
        mutation_id: format!("mutation-{run_id}"),
        mutation_digest: format!("digest-{run_id}"),
        status: EvolutionStatus::Candidate,
        target_file: target_file.to_string(),
        mutation_kind: "add_unit_test".to_string(),
        risk: 0.1,
        score: 8.0,
        useful_change: true,
        non_candidate_reason: None,
        duplicate_rejected: duplicate,
        regression_penalty: 0.0,
        success_bonus: 0.0,
        cargo_check_ok: cargo_test_ok,
        cargo_test_ok,
        cargo_run_ok,
        retained_in_core: false,
        sandbox_destroyed: true,
        stdout_digest: String::new(),
        stderr_digest: String::new(),
        stderr_tail: String::new(),
        timestamp_unix: 1,
    }
}
