use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use eva_runtime_with_task_validator::contracts::{MutationKind, MutationObjective, MutationPlan};
use eva_runtime_with_task_validator::evolution::compute_mutation_digest;
use eva_runtime_with_task_validator::{rank_plans, LearningContext};

#[test]
fn regression_penalty_lowers_priority() {
    let plan = plan("plan:regression", "src/evolution/validator.rs");
    let baseline = rank_plans(std::slice::from_ref(&plan), &LearningContext::default());

    let mut learning = LearningContext::default();
    learning.regressions.push(regression_entry(
        "src/evolution/validator.rs",
        "appendcomment",
        1.5,
    ));

    let penalized = rank_plans(std::slice::from_ref(&plan), &learning);
    assert!(penalized[0].final_priority < baseline[0].final_priority);
    assert_eq!(penalized[0].regression_penalty, 1.5);
}

#[test]
fn success_bonus_raises_priority() {
    let plan = plan("plan:success", "src/evolution/validator.rs");
    let baseline = rank_plans(std::slice::from_ref(&plan), &LearningContext::default());

    let mut learning = LearningContext::default();
    learning.successes.push(success_entry(
        "src/evolution/validator.rs",
        "appendcomment",
        0.75,
    ));

    let boosted = rank_plans(std::slice::from_ref(&plan), &learning);
    assert!(boosted[0].final_priority > baseline[0].final_priority);
    assert_eq!(boosted[0].success_bonus, 0.75);
}

#[test]
fn duplicate_penalty_lowers_priority() {
    let plan = plan("plan:duplicate", "src/evolution/validator.rs");
    let baseline = rank_plans(std::slice::from_ref(&plan), &LearningContext::default());

    let mut learning = LearningContext::default();
    let predicted = eva_runtime_with_task_validator::generate_from_plan(&plan);
    learning
        .dedup_entries
        .push(eva_runtime_with_task_validator::evolution::DedupEntry {
            digest: compute_mutation_digest(&predicted),
            target_file: predicted.target_file,
            mutation_kind: "appendcomment".to_string(),
            score: 1.0,
            useful_change: false,
            run_id: "run-old".to_string(),
            seen_count: 2,
        });

    let penalized = rank_plans(std::slice::from_ref(&plan), &learning);
    assert!(penalized[0].final_priority < baseline[0].final_priority);
    assert!(penalized[0].duplicate_penalty > 0.0);
}

#[test]
fn deterministic_tie_break_works() {
    let learning = LearningContext::default();
    let hypotheses = rank_plans(
        &[
            plan("plan:b", "src/evolution/validator.rs"),
            plan("plan:a", "src/evolution/validator.rs"),
            MutationPlan {
                id: "plan:c".to_string(),
                estimated_risk: 0.10,
                ..plan("plan:c", "src/evolution/validator.rs")
            },
        ],
        &learning,
    );

    assert_eq!(hypotheses[0].plan_id, "plan:c");
    assert_eq!(hypotheses[1].plan_id, "plan:a");
    assert_eq!(hypotheses[2].plan_id, "plan:b");
}

#[test]
fn plan_evolution_prints_learning_fields() {
    let root = temp_crate("phase40b-plan-cli");
    seed_graph(&root);
    seed_learning_memory(&root);

    let output = Command::new(env!("CARGO_BIN_EXE_eva_runtime_with_task_validator"))
        .arg("--plan-evolution")
        .current_dir(&root)
        .output()
        .expect("run plan evolution");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("final_priority="));
    assert!(stdout.contains("regression_penalty="));
    assert!(stdout.contains("success_bonus="));
    assert!(stdout.contains("duplicate_penalty="));
    assert!(stdout.contains("  - "));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn evolve_planned_logs_learning_fields() {
    let root = temp_crate("phase40b-evolve-planned");
    seed_graph(&root);
    seed_learning_memory(&root);

    let output = Command::new(env!("CARGO_BIN_EXE_eva_runtime_with_task_validator"))
        .arg("--evolve-planned")
        .current_dir(&root)
        .output()
        .expect("run planned evolution");

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let log = fs::read_to_string(root.join("memory/evolution.jsonl")).expect("read evolution log");
    assert!(log.contains("\"plan_id\":"));
    assert!(log.contains("\"hypothesis_id\":"));
    assert!(log.contains("\"objective\":"));
    assert!(log.contains("\"regression_penalty\":"));
    assert!(log.contains("\"success_bonus\":"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn forbidden_files_are_never_selected() {
    let root = temp_crate("phase40b-forbidden");
    fs::create_dir_all(root.join("memory")).expect("create memory");
    fs::write(
        root.join("memory/graph.json"),
        r#"{
  "nodes": [
    {"id":"file:Cargo.toml","kind":"File"},
    {"id":"file:src/main.rs","kind":"File"},
    {"id":"file:src/lib.rs","kind":"File"},
    {"id":"file:src/core/state.rs","kind":"File"},
    {"id":"file:src/evolution/validator.rs","kind":"File"}
  ],
  "edges": []
}"#,
    )
    .expect("write graph");

    let output = Command::new(env!("CARGO_BIN_EXE_eva_runtime_with_task_validator"))
        .arg("--plan-evolution")
        .current_dir(&root)
        .output()
        .expect("run plan evolution");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("tests/evolution_generated_tests.rs"));
    assert!(!stdout.contains("Cargo.toml"));
    assert!(!stdout.contains("src/main.rs"));
    assert!(!stdout.contains("src/lib.rs"));
    assert!(!stdout.contains("src/core/state.rs"));

    fs::remove_dir_all(root).expect("cleanup");
}

fn plan(id: &str, target_file: &str) -> MutationPlan {
    MutationPlan {
        id: id.to_string(),
        objective: MutationObjective::ImproveReliability,
        target_file: target_file.to_string(),
        mutation_kind: MutationKind::AppendComment,
        reason: "test ranking".to_string(),
        expected_gain: 0.5,
        estimated_risk: 0.2,
        evidence_weight: 0.1,
        graph_evidence: vec!["pattern:function:test".to_string()],
    }
}

fn regression_entry(
    target_file: &str,
    mutation_kind: &str,
    penalty: f32,
) -> eva_runtime_with_task_validator::evolution::RegressionEntry {
    eva_runtime_with_task_validator::evolution::RegressionEntry {
        pattern_id: "pattern:regression".to_string(),
        target_area: parent(target_file),
        target_file: target_file.to_string(),
        mutation_kind: mutation_kind.to_string(),
        failure_status: "non_useful".to_string(),
        fail_count: 1,
        penalty,
        last_run_id: "run-regression".to_string(),
    }
}

fn success_entry(
    target_file: &str,
    mutation_kind: &str,
    bonus: f32,
) -> eva_runtime_with_task_validator::evolution::SuccessPatternEntry {
    eva_runtime_with_task_validator::evolution::SuccessPatternEntry {
        pattern_id: "pattern:success".to_string(),
        target_area: parent(target_file),
        target_file: target_file.to_string(),
        mutation_kind: mutation_kind.to_string(),
        success_count: 1,
        average_score: 7.0,
        bonus,
        last_run_id: "run-success".to_string(),
    }
}

fn seed_graph(root: &PathBuf) {
    fs::create_dir_all(root.join("memory")).expect("create memory");
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

fn seed_learning_memory(root: &PathBuf) {
    fs::write(
        root.join("memory/regressions.json"),
        r#"[{"pattern_id":"pattern:regression","target_area":"src","target_file":"src/probe.rs","mutation_kind":"appendcomment","failure_status":"non_useful","fail_count":1,"penalty":0.5,"last_run_id":"run-r"}]"#,
    )
    .expect("write regressions");
    fs::write(
        root.join("memory/success_patterns.json"),
        r#"[{"pattern_id":"pattern:success","target_area":"src","target_file":"src/probe.rs","mutation_kind":"appendcomment","success_count":1,"average_score":7.0,"bonus":0.25,"last_run_id":"run-s"}]"#,
    )
    .expect("write success patterns");
    fs::write(
        root.join("memory/mutation_dedup.json"),
        r#"[{"digest":"placeholder","target_file":"src/probe.rs","mutation_kind":"appendcomment","score":1.0,"useful_change":false,"run_id":"run-d","seen_count":2}]"#,
    )
    .expect("write dedup");
    fs::write(
        root.join("memory/metrics.json"),
        r#"{
  "total_runs": 4,
  "passed_runs": 3,
  "failed_runs": 1,
  "candidate_count": 0,
  "replay_passed": 0,
  "promoted_count": 0,
  "average_score": 3.0,
  "last_run_id": "run-last"
}"#,
    )
    .expect("write metrics");
}

fn parent(path: &str) -> String {
    PathBuf::from(path)
        .parent()
        .map(|parent| parent.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

fn temp_crate(name: &str) -> PathBuf {
    let root = temp_dir(name);
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"phase40b_temp\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write cargo");
    fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("write main");
    fs::write(root.join("src/probe.rs"), "pub fn probe() {}\n").expect("write probe");
    root
}

fn temp_dir(name: &str) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_millis();
    std::env::temp_dir().join(format!("{name}-{}-{millis}", std::process::id()))
}
