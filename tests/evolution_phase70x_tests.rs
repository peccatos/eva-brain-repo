use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "evolution_test_support.rs"]
mod evolution_test_support;

use eva_runtime_with_task_validator::contracts::EvolutionStatus;
use eva_runtime_with_task_validator::evolution::{CandidateSummary, ReplayResult};
use eva_runtime_with_task_validator::graph::{GraphEdge, GraphNode};
use eva_runtime_with_task_validator::{
    candidate_lifecycle, print_proof_json, print_proof_report, promotion_blocked_items,
    promotion_ready_items, refresh_promotion_queue, run_demo, supervise_task, DeniedMutationKind,
    EvolutionGraph, EvolutionLogEntry, EvolutionReport, MutationContract, MutationKind,
    MutationObjective, TaskContract,
};

#[test]
fn promotion_queue_classifies_ready_candidate_correctly() {
    let root = temp_runtime_root("phase70-ready");
    seed_autonomy_memory(&root);
    seed_candidate_fixture(
        &root,
        "ready-run",
        MutationKind::AddReplayAssertion,
        "useful",
        "ok",
        true,
        false,
    );

    let queue = refresh_promotion_queue(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("queue");
    assert_eq!(queue.items.len(), 1);
    let ready = promotion_ready_items(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("ready");
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].lifecycle_state, "ready");

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn promotion_queue_rejects_cosmetic_append_comment() {
    let root = temp_runtime_root("phase70-cosmetic");
    seed_autonomy_memory(&root);
    seed_candidate_fixture(
        &root,
        "cosmetic-run",
        MutationKind::AppendComment,
        "cosmetic",
        "ok",
        false,
        false,
    );

    let blocked = promotion_blocked_items(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("blocked");
    assert_eq!(blocked[0].lifecycle_state, "cosmetic_rejected");

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn promotion_queue_rejects_unsafe_and_legacy_classes() {
    let root = temp_runtime_root("phase70-unsafe-legacy");
    seed_autonomy_memory(&root);
    seed_candidate_fixture(
        &root,
        "unsafe-run",
        MutationKind::AddReplayAssertion,
        "unsafe",
        "ok",
        true,
        false,
    );
    seed_candidate_fixture(
        &root,
        "legacy-run",
        MutationKind::AddReplayAssertion,
        "legacy",
        "ok",
        true,
        false,
    );

    let blocked = promotion_blocked_items(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("blocked");
    assert!(blocked
        .iter()
        .any(|item| item.lifecycle_state == "unsafe_rejected"));
    assert!(blocked.iter().any(|item| item.lifecycle_state == "unknown"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn promotion_queue_detects_needs_replay() {
    let root = temp_runtime_root("phase70-needs-replay");
    seed_autonomy_memory(&root);
    seed_candidate_fixture(
        &root,
        "needs-replay-run",
        MutationKind::AddReplayAssertion,
        "useful",
        "not_run",
        true,
        false,
    );

    let blocked = promotion_blocked_items(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("blocked");
    assert_eq!(blocked[0].lifecycle_state, "needs_replay");

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn promotion_queue_detects_already_promoted() {
    let root = temp_runtime_root("phase70-promoted");
    seed_autonomy_memory(&root);
    seed_candidate_fixture(
        &root,
        "promoted-run",
        MutationKind::AddReplayAssertion,
        "useful",
        "ok",
        true,
        true,
    );

    let blocked = promotion_blocked_items(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("blocked");
    assert_eq!(blocked[0].lifecycle_state, "already_promoted");

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn candidate_lifecycle_output_is_deterministic() {
    let root = temp_runtime_root("phase70-lifecycle");
    seed_autonomy_memory(&root);
    seed_candidate_fixture(
        &root,
        "lifecycle-run",
        MutationKind::AddReplayAssertion,
        "useful",
        "ok",
        true,
        false,
    );

    let first = candidate_lifecycle(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "lifecycle-run",
    )
    .expect("first");
    let second = candidate_lifecycle(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "lifecycle-run",
    )
    .expect("second");
    assert_eq!(
        serde_json::to_string_pretty(&first).expect("serialize first"),
        serde_json::to_string_pretty(&second).expect("serialize second")
    );

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn supervised_loop_stops_on_promotion_ready_candidate() {
    let root = temp_runtime_root("phase70-supervise-ready");
    seed_autonomy_memory(&root);
    seed_recombination_graph(&root);
    let path = write_task_file(&root, &replay_task("phase70_supervise_ready"));

    let run = supervise_task(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        path.to_str().unwrap(),
        3,
    )
    .expect("supervise");
    assert_eq!(run.final_status, "promotion_ready_candidate_found");
    assert!(!run.ready_candidate_run_ids.is_empty());
    assert!(!run.auto_promote);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn supervised_loop_adjusts_task_after_zero_yield() {
    let root = temp_runtime_root("phase70-supervise-adjust");
    seed_autonomy_memory(&root);
    seed_recombination_graph(&root);
    let mut task = replay_task("phase70_supervise_adjust");
    task.allowed_targets = vec!["src/sandbox/*".to_string()];
    let path = write_task_file(&root, &task);

    let run = supervise_task(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        path.to_str().unwrap(),
        2,
    )
    .expect("supervise");
    assert!(!run.adjusted_task_paths.is_empty());
    assert!(run.executed_rounds >= 1);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn supervised_loop_stops_after_max_rounds() {
    let root = temp_runtime_root("phase70-supervise-max");
    seed_autonomy_memory(&root);
    seed_recombination_graph(&root);
    let mut task = replay_task("phase70_supervise_max");
    task.allowed_targets = vec!["src/sandbox/*".to_string()];
    let path = write_task_file(&root, &task);

    let run = supervise_task(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        path.to_str().unwrap(),
        1,
    )
    .expect("supervise");
    assert_eq!(run.final_status, "max_rounds_reached");
    assert_eq!(run.executed_rounds, 1);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn supervised_loop_never_auto_promotes() {
    let root = temp_runtime_root("phase70-supervise-no-promote");
    seed_autonomy_memory(&root);
    seed_recombination_graph(&root);
    let path = write_task_file(&root, &replay_task("phase70_supervise_no_promote"));

    let run = supervise_task(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        path.to_str().unwrap(),
        2,
    )
    .expect("supervise");
    assert!(!run.auto_promote);
    let evolution_log = fs::read_to_string(root.join("memory/evolution.jsonl")).expect("log");
    assert!(!evolution_log.contains("\"retained_in_core\":true"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn supervised_loop_preserves_forbidden_targets() {
    let root = temp_runtime_root("phase70-supervise-forbidden");
    seed_autonomy_memory(&root);
    seed_recombination_graph(&root);
    let mut task = replay_task("phase70_supervise_forbidden");
    task.allowed_targets = vec!["src/sandbox/*".to_string()];
    let path = write_task_file(&root, &task);

    let run = supervise_task(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        path.to_str().unwrap(),
        2,
    )
    .expect("supervise");
    if let Some(adjusted) = run.adjusted_task_paths.last() {
        let adjusted_task: TaskContract =
            serde_json::from_str(&fs::read_to_string(adjusted).expect("adjusted task"))
                .expect("parse adjusted task");
        assert!(adjusted_task
            .forbidden_targets
            .contains(&"src/core/*".to_string()));
        assert!(adjusted_task
            .forbidden_targets
            .contains(&"src/main.rs".to_string()));
        assert!(adjusted_task
            .forbidden_targets
            .contains(&"src/lib.rs".to_string()));
        assert!(adjusted_task
            .forbidden_targets
            .contains(&"Cargo.toml".to_string()));
    }

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn proof_report_includes_all_major_capabilities() {
    let root = temp_runtime_root("phase70-proof");
    seed_autonomy_memory(&root);
    seed_candidate_fixture(
        &root,
        "proof-run",
        MutationKind::AddReplayAssertion,
        "useful",
        "ok",
        true,
        false,
    );

    let report = print_proof_report(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("proof report");
    assert!(report.contains("local_corpus_ingestion_support=true"));
    assert!(report.contains("promotion_queue_support=true"));
    assert!(report.contains("supervised_task_support=true"));
    assert!(report.contains("auto_promote=false"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn proof_json_is_deterministic_and_rebuildable() {
    let root = temp_runtime_root("phase70-proof-json");
    seed_autonomy_memory(&root);
    seed_candidate_fixture(
        &root,
        "proof-json-run",
        MutationKind::AddReplayAssertion,
        "useful",
        "ok",
        true,
        false,
    );

    let first = print_proof_json(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("first");
    let second = print_proof_json(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("second");
    assert_eq!(first, second);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn demo_command_does_not_create_sandbox_leaks_or_mutate_source_files() {
    let root = temp_runtime_root("phase70-demo");
    seed_autonomy_memory(&root);
    seed_candidate_fixture(
        &root,
        "demo-run",
        MutationKind::AddReplayAssertion,
        "useful",
        "ok",
        true,
        false,
    );
    let before_main = fs::read_to_string(root.join("src/main.rs")).expect("main");
    let before_lib = fs::read_to_string(root.join("src/lib.rs")).expect("lib");

    let output = run_demo(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("demo");
    assert!(output.contains("eva_status:"));
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

fn temp_runtime_root(name: &str) -> PathBuf {
    let root = evolution_test_support::unique_evolution_root(name);
    fs::create_dir_all(root.join("src")).expect("src");
    fs::create_dir_all(root.join("tests")).expect("tests");
    fs::create_dir_all(root.join("memory")).expect("memory");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"phase70_temp\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\ndoctest = false\n",
    )
    .expect("cargo");
    fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("main");
    fs::write(root.join("src/lib.rs"), "pub fn probe() -> bool { true }\n").expect("lib");
    root
}

fn seed_candidate_fixture(
    root: &Path,
    run_id: &str,
    kind: MutationKind,
    mutation_class: &str,
    replay_status: &str,
    useful_change: bool,
    already_promoted: bool,
) {
    fs::create_dir_all(root.join("memory/candidates")).expect("candidates");
    fs::create_dir_all(root.join("memory/reports")).expect("reports");
    let digest = format!("digest-{run_id}");
    let summary = CandidateSummary {
        run_id: run_id.to_string(),
        mutation_id: format!("mutation-{run_id}"),
        mutation_digest: digest.clone(),
        status: EvolutionStatus::Candidate,
        target_file: "tests/evolution_generated_tests.rs".to_string(),
        mutation_kind: format!("{kind:?}").to_ascii_lowercase(),
        risk: 0.10,
        score: 8.5,
        useful_change,
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
    let mutation = MutationContract {
        id: summary.mutation_id.clone(),
        kind,
        target_file: summary.target_file.clone(),
        search: None,
        replace: None,
        append: Some(format!(
            "#[test]\nfn eva_generated_{run_id}_deterministic() {{ assert!(true); }}\n"
        )),
        reason: "fixture".to_string(),
        expected_gain: 0.5,
        risk: summary.risk,
    };
    let report = EvolutionReport {
        run_id: run_id.to_string(),
        status: EvolutionStatus::Candidate,
        goal_ru: "fixture".to_string(),
        selected_plan_ru: "fixture".to_string(),
        mutation_ru: "fixture".to_string(),
        target_file: summary.target_file.clone(),
        mutation_kind: summary.mutation_kind.clone(),
        hypothesis_id: None,
        source_patterns: Vec::new(),
        avoided_risks: Vec::new(),
        recombination_reason_ru: None,
        portfolio_reason_ru: None,
        selected_strategy: Some("ReplaySafety".to_string()),
        policy_reason_ru: Some("fixture".to_string()),
        mutation_class: mutation_class.to_string(),
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
        replay_status: replay_status.to_string(),
        replay_checked_at: Some(100),
        risk_ru: "ok".to_string(),
        next_step_ru: "ok".to_string(),
    };
    fs::write(
        root.join("memory/candidates")
            .join(format!("{run_id}.summary.json")),
        serde_json::to_string_pretty(&summary).expect("summary"),
    )
    .expect("write summary");
    fs::write(
        root.join("memory/candidates")
            .join(format!("{run_id}.mutation.json")),
        serde_json::to_string_pretty(&mutation).expect("mutation"),
    )
    .expect("write mutation");
    fs::write(
        root.join("memory/reports")
            .join(format!("{run_id}.report.json")),
        serde_json::to_string_pretty(&report).expect("report"),
    )
    .expect("write report");
    fs::write(
        root.join("memory/reports").join(format!("{run_id}.ru.md")),
        "fixture report",
    )
    .expect("write report md");
    if replay_status != "not_run" {
        fs::create_dir_all(root.join("memory/replays")).expect("replays");
        let replay = ReplayResult {
            run_id: run_id.to_string(),
            replay_status: if replay_status == "ok" {
                EvolutionStatus::Candidate
            } else {
                EvolutionStatus::Failed
            },
            score: 8.5,
            matches_stored_summary: replay_status == "ok",
            cargo_check_ok: replay_status == "ok",
            cargo_test_ok: replay_status == "ok",
            cargo_run_ok: replay_status == "ok",
            stdout_digest: String::new(),
            stderr_digest: String::new(),
            stderr_tail: String::new(),
            sandbox_destroyed: true,
            timestamp_unix: 100,
        };
        fs::write(
            root.join("memory/replays").join(format!("{run_id}.json")),
            serde_json::to_string_pretty(&replay).expect("replay"),
        )
        .expect("write replay");
    }
    if already_promoted {
        append_evolution_log(
            &root.join("memory/evolution.jsonl"),
            &EvolutionLogEntry {
                run_id: format!("promoted-{run_id}"),
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
                mutation_id: format!("mutation-{run_id}"),
                mutation_digest: digest,
                status: EvolutionStatus::Promoted,
                target_file: summary.target_file.clone(),
                mutation_kind: summary.mutation_kind.clone(),
                risk: summary.risk,
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
        fs::write(
            root.join("memory/replays").join(format!("seed-{index}.json")),
            format!(
                "{{\"run_id\":\"seed-{index}\",\"replay_status\":\"passed\",\"score\":8.5,\"matches_stored_summary\":true,\"cargo_check_ok\":true,\"cargo_test_ok\":true,\"cargo_run_ok\":true,\"stdout_digest\":\"\",\"stderr_digest\":\"\",\"stderr_tail\":\"\",\"sandbox_destroyed\":true,\"timestamp_unix\":{}}}",
                index + 1
            ),
        )
        .expect("replay");
    }
}

fn seed_recombination_graph(root: &Path) {
    let graph = EvolutionGraph {
        nodes: vec![
            GraphNode {
                id: "file:src/promotion/replay.rs".to_string(),
                kind: "File".to_string(),
            },
            GraphNode {
                id: "file:src/validator.rs".to_string(),
                kind: "File".to_string(),
            },
        ],
        edges: vec![
            GraphEdge {
                from: "pattern:replay".to_string(),
                to: "file:src/promotion/replay.rs".to_string(),
                relation: "supports".to_string(),
            },
            GraphEdge {
                from: "pattern:validator".to_string(),
                to: "file:src/validator.rs".to_string(),
                relation: "supports".to_string(),
            },
        ],
    };
    fs::write(
        root.join("memory/graph.json"),
        serde_json::to_string_pretty(&graph).expect("graph"),
    )
    .expect("write graph");
    fs::write(
        root.join("memory/metrics.json"),
        r#"{"total_runs":12,"passed_runs":12,"failed_runs":0,"candidate_count":12,"replay_passed":4,"promoted_count":5,"average_score":8.5,"last_run_id":"seed-11"}"#,
    )
    .expect("metrics");
}

fn replay_task(task_id: &str) -> TaskContract {
    TaskContract {
        task_id: task_id.to_string(),
        title_ru: "Phase70 task".to_string(),
        goal_ru: "Operator supervised task".to_string(),
        allowed_targets: vec!["tests/*".to_string()],
        forbidden_targets: vec![
            "src/core/*".to_string(),
            "src/main.rs".to_string(),
            "src/lib.rs".to_string(),
            "Cargo.toml".to_string(),
        ],
        preferred_objectives: vec![MutationObjective::ImproveValidation],
        allowed_mutation_kinds: vec![MutationKind::AddReplayAssertion],
        denied_mutation_kinds: vec![
            DeniedMutationKind::DeleteCode,
            DeniedMutationKind::RewriteFunction,
            DeniedMutationKind::FreeDiff,
            DeniedMutationKind::DependencyAdd,
        ],
        cycles: 1,
        require_replay: true,
        require_benchmark: false,
        require_russian_report: true,
        auto_promote: false,
        max_risk: 0.2,
        min_score: 7.0,
        source_corpus_id: Some("corpus_phase70".to_string()),
        created_at: 1,
    }
}

fn write_task_file(root: &Path, task: &TaskContract) -> PathBuf {
    let path = root.join(format!("{}.task.json", task.task_id));
    fs::write(&path, serde_json::to_string_pretty(task).expect("task")).expect("write task");
    path
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
