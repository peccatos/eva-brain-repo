use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "evolution_test_support.rs"]
mod evolution_test_support;

use eva_runtime_with_task_validator::graph::{GraphEdge, GraphNode};
use eva_runtime_with_task_validator::{
    adjust_task_from_campaign, load_policy_feedback, preview_campaign_recombination,
    refresh_evolution_policy, run_bounded_evolution, run_task_from_path,
    select_task_compatible_from_hypotheses, MutationKind, MutationObjective, RecombinedHypothesis,
    TaskContract,
};
use eva_runtime_with_task_validator::{DeniedMutationKind, EvolutionGraph};
use serde_json::Value;

#[test]
fn campaign_recombination_bridge_accepts_task_compatible_hypothesis() {
    let task = replay_task("bridge_accept");
    let hypothesis = hypothesis(
        "h1",
        "addreplayassertion",
        "tests/evolution_generated_tests.rs",
        0.12,
    );
    let (selected, diagnostics) =
        select_task_compatible_from_hypotheses(vec![hypothesis.clone()], &task);
    assert_eq!(
        selected.expect("selected").hypothesis_id,
        hypothesis.hypothesis_id
    );
    assert!(diagnostics.recombination_fallback_used);
}

#[test]
fn bridge_rejects_forbidden_target() {
    let mut task = replay_task("bridge_forbidden");
    task.forbidden_targets.push("tests/*".to_string());
    let hypothesis = hypothesis(
        "h1",
        "addreplayassertion",
        "tests/evolution_generated_tests.rs",
        0.12,
    );
    let (selected, diagnostics) = select_task_compatible_from_hypotheses(vec![hypothesis], &task);
    assert!(selected.is_none());
    assert_eq!(diagnostics.recombination_rejected_by_forbidden_target, 1);
}

#[test]
fn bridge_rejects_disallowed_kind() {
    let mut task = replay_task("bridge_kind");
    task.allowed_mutation_kinds = vec![MutationKind::AddMetricUpdate];
    let hypothesis = hypothesis(
        "h1",
        "addreplayassertion",
        "tests/evolution_generated_tests.rs",
        0.12,
    );
    let (selected, diagnostics) = select_task_compatible_from_hypotheses(vec![hypothesis], &task);
    assert!(selected.is_none());
    assert_eq!(diagnostics.recombination_rejected_by_kind, 1);
}

#[test]
fn bridge_rejects_risk_above_task_max_risk() {
    let mut task = replay_task("bridge_risk");
    task.max_risk = 0.05;
    let hypothesis = hypothesis(
        "h1",
        "addreplayassertion",
        "tests/evolution_generated_tests.rs",
        0.12,
    );
    let (selected, diagnostics) = select_task_compatible_from_hypotheses(vec![hypothesis], &task);
    assert!(selected.is_none());
    assert_eq!(diagnostics.recombination_rejected_by_risk, 1);
}

#[test]
fn bridge_rejects_cosmetic_unsafe_and_legacy_classes() {
    let task = replay_task("bridge_class");
    let hypotheses = vec![
        hypothesis(
            "c1",
            "appendcomment",
            "tests/evolution_generated_tests.rs",
            0.01,
        ),
        hypothesis(
            "c2",
            "deletecode",
            "tests/evolution_generated_tests.rs",
            0.01,
        ),
        hypothesis(
            "c3",
            "legacykind",
            "tests/evolution_generated_tests.rs",
            0.01,
        ),
    ];
    let (selected, diagnostics) = select_task_compatible_from_hypotheses(hypotheses, &task);
    assert!(selected.is_none());
    assert_eq!(diagnostics.recombination_rejected_by_class, 3);
}

#[test]
fn campaign_uses_recombination_fallback_after_zero_accepted_plans() {
    let root = temp_runtime_root("phase60-campaign-fallback");
    seed_autonomy_memory(&root);
    seed_recombination_graph(&root);
    let task = replay_task("phase60_campaign_fallback");
    let path = write_task_file(&root, &task);

    let campaign = run_task_from_path(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        path.to_str().unwrap(),
    )
    .expect("campaign");
    assert!(campaign.recombination_fallback_attempted);
    assert!(campaign.recombination_fallback_used);
    assert_eq!(campaign.accepted_plan_count, 0);
    assert!(campaign.candidate_generated_count > 0);

    let report = fs::read_to_string(
        root.join("memory/campaigns")
            .join(format!("{}.ru.md", campaign.campaign_id)),
    )
    .expect("report");
    assert!(report.contains("## Recombination fallback"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn candidate_recovery_diagnostics_explain_no_candidate_result() {
    let root = temp_runtime_root("phase60-no-candidate");
    seed_autonomy_memory(&root);
    seed_recombination_graph(&root);
    let mut task = replay_task("phase60_no_candidate");
    task.allowed_targets = vec!["src/sandbox/*".to_string()];
    let path = write_task_file(&root, &task);

    let campaign = run_task_from_path(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        path.to_str().unwrap(),
    )
    .expect("campaign");
    assert_eq!(campaign.useful_candidates, 0);
    assert!(campaign.candidate_recovery_reason.is_some());

    let adjustment =
        adjust_task_from_campaign(root.join("memory").to_str().unwrap(), &campaign.campaign_id)
            .expect("adjustment");
    assert!(Path::new(&adjustment.adjusted_task_path).exists());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn policy_feedback_updates_after_zero_yield_campaign() {
    let root = temp_runtime_root("phase60-feedback");
    seed_autonomy_memory(&root);
    seed_recombination_graph(&root);
    let mut task = replay_task("phase60_feedback");
    task.allowed_targets = vec!["src/sandbox/*".to_string()];
    let path = write_task_file(&root, &task);

    let campaign = run_task_from_path(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        path.to_str().unwrap(),
    )
    .expect("campaign");
    let feedback = load_policy_feedback(root.join("memory").to_str().unwrap()).expect("feedback");
    assert_eq!(feedback.last_campaign_id, campaign.campaign_id);
    assert!(feedback.zero_yield_count > 0);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn policy_feedback_affects_deterministic_policy_ranking() {
    let root = temp_runtime_root("phase60-policy");
    seed_autonomy_memory(&root);
    seed_recombination_graph(&root);
    let before = refresh_evolution_policy(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        None,
    )
    .expect("policy before");
    fs::write(
        root.join("memory/policy_feedback.json"),
        r#"{"zero_yield_count":3,"repeated_task_constraints_too_narrow":3,"repeated_duplicate_payload":0,"repeated_below_min_score":0,"failing_strategy_counts":{},"last_campaign_id":"x","updated_at":1}"#,
    )
    .expect("feedback");
    let after = refresh_evolution_policy(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        None,
    )
    .expect("policy after");
    assert_ne!(before.policy_reason_ru, after.policy_reason_ru);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn evolve_bounded_writes_json_and_ru_report_and_never_auto_promotes() {
    let root = temp_runtime_root("phase60-bounded");
    seed_autonomy_memory(&root);
    seed_recombination_graph(&root);
    let task = replay_task("phase60_bounded");
    let path = write_task_file(&root, &task);

    let bounded = run_bounded_evolution(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        path.to_str().unwrap(),
        3,
    )
    .expect("bounded");
    assert!(!bounded.auto_promote);
    assert!(!bounded.campaign_ids.is_empty());
    assert!(root
        .join("memory/bounded_runs")
        .join(format!("{}.json", bounded.bounded_run_id))
        .exists());
    assert!(root
        .join("memory/bounded_runs")
        .join(format!("{}.ru.md", bounded.bounded_run_id))
        .exists());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn evolve_bounded_stops_on_promotion_ready_candidate_and_respects_max_cycles() {
    let root = temp_runtime_root("phase60-bounded-stop");
    seed_autonomy_memory(&root);
    seed_recombination_graph(&root);
    let task = replay_task("phase60_bounded_stop");
    let path = write_task_file(&root, &task);

    let bounded = run_bounded_evolution(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        path.to_str().unwrap(),
        20,
    )
    .expect("bounded");
    assert!(bounded.executed_cycles <= 10);
    assert!(!bounded.promotion_ready_run_ids.is_empty());
    assert!(bounded.stopped_early);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn bounded_run_reconciles_campaign_report_after_replay_review_recovery() {
    let root = temp_runtime_root("phase60-report-consistency");
    seed_autonomy_memory(&root);
    seed_recombination_graph(&root);
    let task = replay_task("phase60_report_consistency");
    let path = write_task_file(&root, &task);

    let bounded = run_bounded_evolution(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        path.to_str().unwrap(),
        2,
    )
    .expect("bounded");
    let campaign_id = bounded.campaign_ids.first().expect("campaign id");
    let report = fs::read_to_string(
        root.join("memory/campaigns")
            .join(format!("{campaign_id}.ru.md")),
    )
    .expect("campaign report");
    let campaign_json = fs::read_to_string(
        root.join("memory/campaigns")
            .join(format!("{campaign_id}.json")),
    )
    .expect("campaign json");
    let campaign: Value = serde_json::from_str(&campaign_json).expect("campaign parse");

    assert!(!report.contains("replay_not_ok"));
    assert!(report.contains("Не пройдено: 0"));
    assert_eq!(campaign["replay_failed"].as_u64(), Some(0));
    assert_eq!(
        campaign["candidate_rejected_failed_replay"].as_u64(),
        Some(0)
    );
    assert!(
        campaign["promotion_ready_candidates"].as_u64().unwrap_or(0) > 0,
        "campaign json={campaign_json}"
    );

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn campaign_recombine_preview_creates_no_sandbox_and_respects_constraints() {
    let root = temp_runtime_root("phase60-preview");
    seed_autonomy_memory(&root);
    seed_recombination_graph(&root);
    let mut task = replay_task("phase60_preview");
    task.allowed_targets = vec!["tests/*".to_string()];
    let path = write_task_file(&root, &task);

    let preview = preview_campaign_recombination(
        root.join("memory").to_str().unwrap(),
        &eva_runtime_with_task_validator::evolution::load_task_contract(&path).expect("task"),
    )
    .expect("preview");
    assert!(preview.diagnostics.recombination_fallback_attempted);
    assert!(
        !root.join("memory/evolution.jsonl").exists()
            || fs::read_to_string(root.join("memory/evolution.jsonl"))
                .unwrap()
                .contains("seed-")
    );

    let cli = run_ok(
        &root,
        &["--campaign-recombine-preview", path.to_str().unwrap()],
    );
    assert!(cli.contains("recombination_fallback_attempted"));

    fs::remove_dir_all(root).expect("cleanup");
}

fn temp_runtime_root(name: &str) -> PathBuf {
    let root = evolution_test_support::unique_evolution_root(name);
    fs::create_dir_all(root.join("src")).expect("src");
    fs::create_dir_all(root.join("tests")).expect("tests");
    fs::create_dir_all(root.join("memory")).expect("memory");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"phase60_temp\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\ndoctest = false\n",
    )
    .expect("cargo");
    fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("main");
    fs::write(root.join("src/lib.rs"), "pub fn probe() -> bool { true }\n").expect("lib");
    root
}

fn replay_task(task_id: &str) -> TaskContract {
    TaskContract {
        task_id: task_id.to_string(),
        title_ru: "Phase60 task".to_string(),
        goal_ru: "Fallback campaign task".to_string(),
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
        source_corpus_id: Some("corpus_phase60".to_string()),
        created_at: 1,
    }
}

fn hypothesis(id: &str, kind: &str, target: &str, risk: f32) -> RecombinedHypothesis {
    RecombinedHypothesis {
        hypothesis_id: id.to_string(),
        source_patterns: vec!["file:src/promotion/replay.rs".to_string()],
        avoided_risks: Vec::new(),
        target_objective: "ImproveReplayability".to_string(),
        suggested_mutation_kind: kind.to_string(),
        suggested_target: target.to_string(),
        reason_ru: "test".to_string(),
        portfolio_reason_ru: "test".to_string(),
        selected_strategy: "ReplaySafety".to_string(),
        policy_reason_ru: "test".to_string(),
        expected_gain: 0.5,
        estimated_risk: risk,
        confidence: 0.8,
        diversity_bonus: 0.0,
        saturation_penalty: 0.0,
        repeated_target_penalty: 0.0,
        final_recombination_score: 0.5,
        strategy_bonus: 0.0,
        strategy_saturation_penalty: 0.0,
        quality_bonus: 0.0,
        novelty_score: 0.0,
        useful_delta_score: 0.0,
        duplicate_suppression_score: 0.0,
        regression_avoidance_score: 0.0,
        coverage_proxy_score: 0.0,
        quality_score: 0.0,
        final_strategy_score: 0.5,
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

fn write_task_file(root: &Path, task: &TaskContract) -> PathBuf {
    let path = root.join(format!("{}.task.json", task.task_id));
    fs::write(&path, serde_json::to_string_pretty(task).expect("task")).expect("write task");
    path
}

fn run_ok(root: &Path, args: &[&str]) -> String {
    let output = evolution_test_support::eva_command(root)
        .args(args)
        .output()
        .expect("run");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("stdout")
}
