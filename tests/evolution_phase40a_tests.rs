use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use eva_runtime_with_task_validator::contracts::{EvolutionLogEntry, EvolutionStatus};
use eva_runtime_with_task_validator::evolution::memory::load_candidate_summary;
use eva_runtime_with_task_validator::evolution::{
    compute_mutation_digest, learning_summary, load_dedup_entries, load_regressions,
    load_success_patterns, record_success_pattern, refresh_metrics,
};
use eva_runtime_with_task_validator::{
    load_metrics, run_evolution_cycle_with_memory, MutationContract, MutationKind,
};

fn useful_entry(run_id: &str) -> EvolutionLogEntry {
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
        diversity_bonus: 0.0,
        saturation_penalty: 0.0,
        repeated_target_penalty: 0.0,
        final_recombination_score: 0.0,
        mutation_id: "useful-mutation".to_string(),
        mutation_digest: "digest-useful".to_string(),
        status: EvolutionStatus::Candidate,
        target_file: "src/probe.rs".to_string(),
        mutation_kind: "replacetext".to_string(),
        risk: 0.1,
        score: 7.0,
        useful_change: true,
        non_candidate_reason: None,
        duplicate_rejected: false,
        regression_penalty: 0.0,
        success_bonus: 0.0,
        cargo_check_ok: true,
        cargo_test_ok: true,
        cargo_run_ok: true,
        retained_in_core: false,
        sandbox_destroyed: true,
        stdout_digest: String::new(),
        stderr_digest: String::new(),
        stderr_tail: String::new(),
        timestamp_unix: 1,
    }
}

#[test]
fn regression_memory_updates_after_non_useful_mutation() {
    let root = temp_crate("phase40a-regression");

    run_evolution_cycle_with_memory(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("first evolution run should pass");

    let regressions =
        load_regressions(root.join("memory").to_str().unwrap()).expect("load regressions");
    assert_eq!(regressions.len(), 1);
    assert_eq!(regressions[0].failure_status, "non_useful");
    assert!(regressions[0].penalty > 0.0);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn success_memory_updates_only_for_useful_candidate() {
    let memory = temp_dir("phase40a-success");
    fs::create_dir_all(&memory).expect("create memory");

    let useful = useful_entry("run-useful");
    record_success_pattern(memory.to_str().unwrap(), &useful).expect("record useful success");

    let successes = load_success_patterns(memory.to_str().unwrap()).expect("load success patterns");
    assert_eq!(successes.len(), 1);
    assert_eq!(successes[0].success_count, 1);

    let old_summary_path = memory.join("candidates").join("legacy.summary.json");
    fs::create_dir_all(old_summary_path.parent().unwrap()).expect("create candidates dir");
    fs::write(
        &old_summary_path,
        r#"{
  "run_id":"legacy",
  "mutation_id":"legacy-mutation",
  "status":"candidate",
  "target_file":"src/probe.rs",
  "mutation_kind":"replacetext",
  "risk":0.1,
  "score":6.0,
  "cargo_check_ok":true,
  "cargo_test_ok":true,
  "cargo_run_ok":true,
  "stdout_digest":"",
  "stderr_digest":"",
  "stderr_tail":"",
  "timestamp_unix":1
}"#,
    )
    .expect("write legacy summary");
    let loaded = load_candidate_summary(memory.to_str().unwrap(), "legacy").expect("load legacy");
    assert!(!loaded.useful_change);
    assert_eq!(loaded.non_candidate_reason, None);

    fs::remove_dir_all(memory).expect("cleanup");
}

#[test]
fn duplicate_bad_mutation_is_rejected_before_sandbox() {
    let root = temp_crate("phase40a-duplicate");
    let memory_root = root.join("memory");

    run_evolution_cycle_with_memory(root.to_str().unwrap(), memory_root.to_str().unwrap())
        .expect("first evolution run");
    let before_candidates = candidate_summary_count(&memory_root);

    let err =
        run_evolution_cycle_with_memory(root.to_str().unwrap(), memory_root.to_str().unwrap())
            .expect_err("second identical bad mutation should reject");
    assert!(err.contains("duplicate bad mutation"));
    assert_eq!(before_candidates, candidate_summary_count(&memory_root));

    let logs = fs::read_to_string(memory_root.join("evolution.jsonl")).expect("read logs");
    assert!(
        logs.contains("\"duplicate_rejected\":true")
            || logs.contains("\"duplicate_rejected\": true")
    );
    let dedup_entries = load_dedup_entries(memory_root.to_str().unwrap()).expect("load dedup");
    assert_eq!(dedup_entries.len(), 1);
    assert_eq!(dedup_entries[0].seen_count, 2);
    assert_eq!(sandbox_entries(&root), 0);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn mutation_digest_is_deterministic() {
    let mutation = MutationContract {
        id: "digest-test".to_string(),
        kind: MutationKind::ReplaceText,
        target_file: "src/probe.rs".to_string(),
        search: Some("a".to_string()),
        replace: Some("b".to_string()),
        append: None,
        reason: "digest".to_string(),
        expected_gain: 0.2,
        risk: 0.1,
    };
    let left = compute_mutation_digest(&mutation);
    let right = compute_mutation_digest(&mutation);
    assert_eq!(left, right);
}

#[test]
fn metrics_refresh_reconciles_candidate_count_with_files_on_disk() {
    let memory = temp_dir("phase40a-metrics");
    fs::create_dir_all(memory.join("candidates")).expect("create candidates");
    fs::write(
        memory.join("metrics.json"),
        r#"{
  "total_runs": 9,
  "passed_runs": 9,
  "failed_runs": 0,
  "candidate_count": 99,
  "replay_passed": 0,
  "promoted_count": 0,
  "average_score": 9.0,
  "last_run_id": "stale"
}"#,
    )
    .expect("write stale metrics");
    fs::write(
        memory.join("evolution.jsonl"),
        serde_json::to_string(&useful_entry("run-a")).expect("serialize log"),
    )
    .expect("write log");
    for run_id in ["run-a", "run-b"] {
        fs::write(
            memory
                .join("candidates")
                .join(format!("{run_id}.summary.json")),
            format!(
                r#"{{
  "run_id":"{run_id}",
  "mutation_id":"m",
  "status":"candidate",
  "target_file":"src/probe.rs",
  "mutation_kind":"replacetext",
  "risk":0.1,
  "score":6.0,
  "cargo_check_ok":true,
  "cargo_test_ok":true,
  "cargo_run_ok":true,
  "stdout_digest":"",
  "stderr_digest":"",
  "stderr_tail":"",
  "timestamp_unix":1
}}"#
            ),
        )
        .expect("write summary");
    }
    let refreshed = refresh_metrics(memory.to_str().unwrap()).expect("refresh metrics");
    assert_eq!(refreshed.candidate_count, 2);
    let loaded = load_metrics(memory.to_str().unwrap()).expect("load refreshed metrics");
    assert_eq!(loaded.candidate_count, 2);

    fs::remove_dir_all(memory).expect("cleanup");
}

#[test]
fn learning_summary_prints_without_panic() {
    let memory = temp_dir("phase40a-learning");
    fs::create_dir_all(&memory).expect("create memory");
    fs::write(
        memory.join("regressions.json"),
        r#"[{"pattern_id":"p","target_area":"src/evolution","target_file":"src/evolution/validator.rs","mutation_kind":"appendcomment","failure_status":"non_useful","fail_count":1,"penalty":0.5,"last_run_id":"r1"}]"#,
    )
    .expect("write regressions");
    fs::write(
        memory.join("success_patterns.json"),
        r#"[{"pattern_id":"s","target_area":"src/tests","target_file":"tests/eva.rs","mutation_kind":"addtestskeleton","success_count":1,"average_score":7.0,"bonus":0.25,"last_run_id":"r2"}]"#,
    )
    .expect("write success patterns");
    fs::write(
        memory.join("mutation_dedup.json"),
        r#"[{"digest":"d","target_file":"src/probe.rs","mutation_kind":"replacetext","score":7.0,"useful_change":true,"run_id":"r2","seen_count":1}]"#,
    )
    .expect("write dedup");

    let summary = learning_summary(memory.to_str().unwrap()).expect("learning summary");
    assert!(summary.contains("total regression patterns: 1"));
    assert!(summary.contains("mutation dedup count: 1"));

    fs::remove_dir_all(memory).expect("cleanup");
}

#[test]
fn old_logs_with_missing_new_fields_still_load() {
    let memory = temp_dir("phase40a-legacy-logs");
    fs::create_dir_all(&memory).expect("create memory");
    fs::write(
        memory.join("evolution.jsonl"),
        r#"{"run_id":"legacy","mutation_id":"old","status":"passed","target_file":"src/probe.rs","mutation_kind":"appendcomment","risk":0.1,"score":2.0,"cargo_check_ok":true,"cargo_test_ok":true,"cargo_run_ok":true,"retained_in_core":false,"sandbox_destroyed":true,"stdout_digest":"","stderr_digest":"","stderr_tail":"","timestamp_unix":1}"#,
    )
    .expect("write legacy log");
    let metrics = refresh_metrics(memory.to_str().unwrap()).expect("refresh legacy metrics");
    assert_eq!(metrics.total_runs, 1);
    assert_eq!(metrics.passed_runs, 1);

    fs::remove_dir_all(memory).expect("cleanup");
}

fn temp_crate(name: &str) -> PathBuf {
    let root = temp_dir(name);
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::create_dir_all(root.join("memory")).expect("create memory");
    fs::create_dir_all(root.join("sandboxes")).expect("create sandboxes");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"phase40a_temp\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write cargo");
    fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("write main");
    fs::write(root.join("src/probe.rs"), "pub fn probe() {}\n").expect("write probe");
    fs::write(root.join("src/runtime_cycle.rs"), "pub fn cycle() {}\n")
        .expect("write runtime cycle");
    root
}

fn temp_dir(name: &str) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_millis();
    std::env::temp_dir().join(format!("{name}-{}-{millis}", std::process::id()))
}

fn candidate_summary_count(memory_root: &PathBuf) -> usize {
    fs::read_dir(memory_root.join("candidates"))
        .map(|entries| entries.count())
        .unwrap_or(0)
}

fn sandbox_entries(root: &PathBuf) -> usize {
    fs::read_dir(root.join("sandboxes"))
        .map(|entries| entries.count())
        .unwrap_or(0)
}
