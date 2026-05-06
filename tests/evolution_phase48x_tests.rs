use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use eva_runtime_with_task_validator::{
    promote_candidate, propose_mutation_plans_for_task, review_candidate, validate_task_contract,
    DeniedMutationKind, MutationKind, MutationObjective, TaskContract,
};

#[test]
fn task_contract_validates_safe_task() {
    validate_task_contract(&safe_task("safe-task", 2)).expect("safe task");
}

#[test]
fn task_contract_rejects_auto_promote_true() {
    let mut task = safe_task("auto-promote-task", 2);
    task.auto_promote = true;
    let error = validate_task_contract(&task).expect_err("auto promote must fail");
    assert!(error.contains("auto_promote"));
}

#[test]
fn task_contract_rejects_missing_hard_forbidden_targets() {
    let mut task = safe_task("missing-forbidden", 2);
    task.forbidden_targets = vec!["src/core/*".to_string()];
    let error = validate_task_contract(&task).expect_err("missing forbidden targets");
    assert!(error.contains("missing hard forbidden target"));
}

#[test]
fn task_contract_rejects_cycles_above_hundred() {
    let task = safe_task("too-many-cycles", 101);
    let error = validate_task_contract(&task).expect_err("cycles should fail");
    assert!(error.contains("<= 100"));
}

#[test]
fn campaign_runs_n_constrained_cycles() {
    let root = temp_crate("phase48x-campaign-cycles");
    seed_graph_basic(&root);
    seed_autonomy_ready_state(&root);
    let task = safe_task("campaign-cycles", 2);
    let task_path = write_task(&root, &task);

    let output = run_ok(&root, &["--run-task", task_path.to_str().unwrap()]);
    assert!(output.contains("\"total_cycles\": 2"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn campaign_writes_json_and_russian_markdown() {
    let root = temp_crate("phase48x-campaign-report");
    seed_graph_basic(&root);
    seed_autonomy_ready_state(&root);
    let mut task = safe_task("campaign-report", 1);
    task.require_replay = true;
    let task_path = write_task(&root, &task);

    run_ok(&root, &["--run-task", task_path.to_str().unwrap()]);
    let campaign_dir = root.join("memory/campaigns");
    let entries = fs::read_dir(&campaign_dir).expect("campaign dir");
    let paths = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    assert!(paths
        .iter()
        .any(|path| path.to_string_lossy().ends_with(".json")));
    assert!(paths
        .iter()
        .any(|path| path.to_string_lossy().ends_with(".ru.md")));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn campaign_does_not_auto_promote() {
    let root = temp_crate("phase48x-campaign-no-promote");
    seed_graph_basic(&root);
    seed_autonomy_ready_state(&root);
    let task = safe_task("campaign-no-promote", 1);
    let task_path = write_task(&root, &task);

    run_ok(&root, &["--run-task", task_path.to_str().unwrap()]);
    let log = fs::read_to_string(root.join("memory/evolution.jsonl")).expect("read evolution log");
    assert!(!log.contains("\"retained_in_core\":true"));
    assert!(!root.join("tests/evolution_generated_tests.rs").exists());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn campaign_reports_promotion_blockers() {
    let root = temp_crate("phase48x-campaign-blockers");
    seed_graph_basic(&root);
    seed_autonomy_ready_state(&root);
    let mut task = safe_task("campaign-blockers", 1);
    task.require_replay = false;
    let task_path = write_task(&root, &task);

    run_ok(&root, &["--run-task", task_path.to_str().unwrap()]);
    let report = run_ok(&root, &["--last-campaign-report"]);
    assert!(report.contains("replay_not_ok"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn task_aware_planner_respects_allowed_targets() {
    let root = temp_crate("phase48x-planner-targets");
    seed_graph_with_metrics(&root);
    let mut task = safe_task("planner-targets", 1);
    task.allowed_targets = vec!["src/evolution/metrics.rs".to_string()];
    task.preferred_objectives = vec![MutationObjective::ImproveGraphMemory];
    task.allowed_mutation_kinds = vec![MutationKind::AddMetricUpdate];

    let plans = propose_mutation_plans_for_task(root.join("memory").to_str().unwrap(), Some(&task))
        .expect("task-aware plans");
    assert!(!plans.is_empty());
    assert!(plans
        .iter()
        .all(|plan| plan.target_file == "src/evolution/metrics.rs"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn task_aware_planner_respects_max_risk() {
    let root = temp_crate("phase48x-planner-risk");
    seed_graph_with_metrics(&root);
    let mut task = safe_task("planner-risk", 1);
    task.max_risk = 0.05;

    let plans = propose_mutation_plans_for_task(root.join("memory").to_str().unwrap(), Some(&task))
        .expect("task-aware plans");
    assert!(plans.is_empty());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn review_candidate_reports_already_promoted() {
    let root = temp_crate("phase48x-already-promoted");
    seed_graph_basic(&root);
    run_ok(&root, &["--evolve-planned"]);
    let run_id = latest_candidate_run_id(&root);
    promote_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        &run_id,
    )
    .expect("promote candidate");

    let review = review_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        &run_id,
    )
    .expect("review promoted candidate");
    assert!(review
        .promotion_blockers
        .contains(&"already_promoted".to_string()));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn review_candidate_reports_target_already_contains_payload() {
    let root = temp_crate("phase48x-target-contains-payload");
    seed_graph_basic(&root);
    run_ok(&root, &["--evolve-planned"]);
    let run_id = latest_candidate_run_id(&root);
    promote_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        &run_id,
    )
    .expect("promote candidate");

    let review = review_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        &run_id,
    )
    .expect("review promoted candidate");
    assert!(review
        .promotion_blockers
        .contains(&"target_already_contains_payload".to_string()));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn list_candidates_includes_blocker_reason() {
    let root = temp_crate("phase48x-list-reason");
    seed_graph_basic(&root);
    seed_autonomy_ready_state(&root);
    run_ok(&root, &["--evolve-planned"]);

    let output = run_ok(&root, &["--list-candidates"]);
    assert!(output.contains("reason=replay_not_ok"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn distill_patterns_writes_local_pattern_summary() {
    let root = temp_crate("phase48x-distill");
    seed_graph_basic(&root);
    run_ok(&root, &["--evolve-planned"]);

    run_ok(&root, &["--distill-patterns"]);
    let path = root.join("memory/patterns/local_distilled_patterns.json");
    assert!(path.exists());
    let contents = fs::read_to_string(path).expect("read pattern summary");
    assert!(contents.contains("top_successful_mutation_kinds"));

    fs::remove_dir_all(root).expect("cleanup");
}

fn safe_task(task_id: &str, cycles: usize) -> TaskContract {
    TaskContract {
        task_id: task_id.to_string(),
        title_ru: "Тестовая задача".to_string(),
        goal_ru: "Проверить bounded evolution campaign".to_string(),
        allowed_targets: vec!["tests/*".to_string()],
        forbidden_targets: vec![
            "src/core/*".to_string(),
            "src/main.rs".to_string(),
            "src/lib.rs".to_string(),
            "Cargo.toml".to_string(),
        ],
        preferred_objectives: vec![MutationObjective::ImproveReliability],
        allowed_mutation_kinds: vec![MutationKind::AddUnitTest],
        denied_mutation_kinds: vec![
            DeniedMutationKind::DeleteCode,
            DeniedMutationKind::RewriteFunction,
            DeniedMutationKind::FreeDiff,
            DeniedMutationKind::DependencyAdd,
        ],
        cycles,
        require_replay: false,
        require_benchmark: false,
        require_russian_report: true,
        auto_promote: false,
        max_risk: 0.2,
        min_score: 5.0,
        source_corpus_id: None,
        created_at: 1,
    }
}

fn write_task(root: &PathBuf, task: &TaskContract) -> PathBuf {
    let path = root.join(format!("{}.task.json", task.task_id));
    fs::write(
        &path,
        serde_json::to_string_pretty(task).expect("serialize task"),
    )
    .expect("write task");
    path
}

fn run_ok(root: &PathBuf, args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_eva_runtime_with_task_validator"))
        .args(args)
        .current_dir(root)
        .output()
        .expect("run command");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn latest_candidate_run_id(root: &PathBuf) -> String {
    let mut summaries = fs::read_dir(root.join("memory/candidates"))
        .expect("candidate dir")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.to_string_lossy().ends_with(".summary.json"))
        .collect::<Vec<_>>();
    summaries.sort();
    summaries
        .last()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .map(|name| name.trim_end_matches(".summary.json").to_string())
        .expect("latest candidate run id")
}

fn temp_crate(name: &str) -> PathBuf {
    let root = temp_dir(name);
    fs::create_dir_all(root.join("src/evolution")).expect("create src evolution");
    fs::create_dir_all(root.join("memory")).expect("create memory");
    fs::create_dir_all(root.join("sandboxes")).expect("create sandboxes");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"phase48x_temp\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write cargo");
    fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("write main");
    fs::write(root.join("src/probe.rs"), "pub fn probe() {}\n").expect("write probe");
    fs::write(root.join("src/runtime_cycle.rs"), "pub fn cycle() {}\n")
        .expect("write runtime cycle");
    fs::write(
        root.join("src/evolution/metrics.rs"),
        "pub fn metrics_probe() -> usize { 1 }\n",
    )
    .expect("write metrics file");
    root
}

fn seed_graph_with_metrics(root: &PathBuf) {
    fs::write(
        root.join("memory/graph.json"),
        r#"{
  "nodes": [
    {"id":"file:src/probe.rs","kind":"File"},
    {"id":"file:src/evolution/metrics.rs","kind":"File"},
    {"id":"pattern:function:probe","kind":"Pattern"},
    {"id":"pattern:function:metrics_probe","kind":"Pattern"}
  ],
  "edges": [
    {"from":"pattern:function:probe","to":"file:src/probe.rs","relation":"found_in"},
    {"from":"pattern:function:metrics_probe","to":"file:src/evolution/metrics.rs","relation":"found_in"}
  ]
}"#,
    )
    .expect("write graph");
}

fn seed_graph_basic(root: &PathBuf) {
    fs::write(
        root.join("memory/graph.json"),
        r#"{
  "nodes": [
    {"id":"file:src/probe.rs","kind":"File"},
    {"id":"pattern:function:probe","kind":"Pattern"}
  ],
  "edges": [
    {"from":"pattern:function:probe","to":"file:src/probe.rs","relation":"found_in"}
  ]
}"#,
    )
    .expect("write graph");
}

fn seed_autonomy_ready_state(root: &PathBuf) {
    let memory = root.join("memory");
    let mut lines = Vec::new();
    for index in 0..10 {
        lines.push(format!(
            "{{\"run_id\":\"seed-{index}\",\"mutation_id\":\"seed-{index}\",\"status\":\"candidate\",\"target_file\":\"tests/evolution_generated_tests.rs\",\"mutation_kind\":\"addunittest\",\"risk\":0.1,\"score\":10.0,\"useful_change\":true,\"duplicate_rejected\":false,\"regression_penalty\":0.0,\"success_bonus\":0.0,\"cargo_check_ok\":true,\"cargo_test_ok\":true,\"cargo_run_ok\":true,\"retained_in_core\":false,\"sandbox_destroyed\":true,\"stdout_digest\":\"\",\"stderr_digest\":\"\",\"stderr_tail\":\"\",\"timestamp_unix\":1}}"
        ));
    }
    fs::write(
        memory.join("evolution.jsonl"),
        format!("{}\n", lines.join("\n")),
    )
    .expect("write evolution log");
    fs::create_dir_all(memory.join("replays")).expect("replays dir");
    for index in 0..3 {
        fs::write(
            memory
                .join("replays")
                .join(format!("seed-replay-{index}.json")),
            r#"{
  "run_id":"seed",
  "replay_status":"candidate",
  "score":10.0,
  "matches_stored_summary":true,
  "cargo_check_ok":true,
  "cargo_test_ok":true,
  "cargo_run_ok":true,
  "stdout_digest":"",
  "stderr_digest":"",
  "stderr_tail":"",
  "sandbox_destroyed":true,
  "timestamp_unix":1
}"#,
        )
        .expect("write replay");
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_millis();
    std::env::temp_dir().join(format!("{name}-{}-{millis}", std::process::id()))
}
