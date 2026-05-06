use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use eva_runtime_with_task_validator::contracts::{MutationKind, MutationObjective, MutationPlan};
use eva_runtime_with_task_validator::{
    extract_rust_ast, generate_from_plan, ingest_repo_patterns, propose_mutation_plans, rank_plans,
    validate_mutation, LearningContext,
};

#[test]
fn ast_extractor_reads_rust_items() {
    let source = r#"
use std::fmt;
mod nested {
    pub struct Inner;
}
pub struct Probe;
enum Mode { A }
fn helper() {}
#[test]
fn helper_test() {}
"#;

    let ast = extract_rust_ast(source).expect("extract ast");

    assert!(ast.modules.contains(&"nested".to_string()));
    assert!(ast.functions.contains(&"helper".to_string()));
    assert!(ast.structs.contains(&"Probe".to_string()));
    assert!(ast.structs.contains(&"Inner".to_string()));
    assert!(ast.enums.contains(&"Mode".to_string()));
    assert!(ast.use_imports.contains(&"std::fmt".to_string()));
    assert!(ast.test_functions.contains(&"helper_test".to_string()));
}

#[test]
fn graph_analyzer_does_not_target_forbidden_files() {
    let memory = temp_dir("phase37-analyzer");
    fs::create_dir_all(&memory).expect("create memory");
    fs::write(
        memory.join("graph.json"),
        r#"{
  "nodes": [
    {"id":"file:src/main.rs","kind":"File"},
    {"id":"file:src/lib.rs","kind":"File"},
    {"id":"file:src/core/belief_state.rs","kind":"File"},
    {"id":"file:src/evolution/validator.rs","kind":"File"}
  ],
  "edges": [
    {"from":"pattern:function:validate_mutation","to":"file:src/evolution/validator.rs","relation":"found_in"}
  ]
}"#,
    )
    .expect("write graph");

    let plans = propose_mutation_plans(memory.to_str().unwrap()).expect("plans");
    assert!(!plans.is_empty());
    assert!(plans.iter().all(|plan| {
        !plan.target_file.starts_with("src/core/")
            && plan.target_file != "src/main.rs"
            && plan.target_file != "src/lib.rs"
            && !plan.target_file.ends_with("Cargo.toml")
    }));

    fs::remove_dir_all(memory).expect("cleanup");
}

#[test]
fn hypothesis_ranking_is_deterministic() {
    let plans = vec![
        plan("plan:b", 0.5, 0.2, 0.1),
        plan("plan:a", 0.5, 0.2, 0.1),
        plan("plan:c", 0.7, 0.1, 0.0),
    ];

    let learning = LearningContext::default();
    let first = rank_plans(&plans, &learning);
    let second = rank_plans(&plans, &learning);

    assert_eq!(first, second);
    assert_eq!(first[0].plan_id, "plan:c");
    assert_eq!(first[1].plan_id, "plan:a");
}

#[test]
fn generate_from_plan_returns_validator_safe_mutation() {
    let plan = MutationPlan {
        id: "plan:test-skeleton".to_string(),
        objective: MutationObjective::ImproveTests,
        target_file: "tests/eva_generated_phase37_tests.rs".to_string(),
        mutation_kind: MutationKind::AddTestSkeleton,
        reason: "add bounded test skeleton".to_string(),
        expected_gain: 0.5,
        estimated_risk: 0.1,
        evidence_weight: 0.2,
        graph_evidence: vec!["pattern:test:existing".to_string()],
    };

    let mutation = generate_from_plan(&plan);
    validate_mutation(&mutation).expect("validator-safe mutation");
    assert!(matches!(
        mutation.kind,
        MutationKind::AddTestSkeleton | MutationKind::AddUnitTest
    ));
    assert!(mutation.target_file.starts_with("tests/"));
}

#[test]
fn plan_evolution_cli_creates_no_sandbox() {
    let root = temp_crate("phase37-plan-cli");
    seed_graph(&root);

    let output = Command::new(env!("CARGO_BIN_EXE_eva_runtime_with_task_validator"))
        .arg("--plan-evolution")
        .current_dir(&root)
        .output()
        .expect("run plan evolution");

    assert!(output.status.success());
    assert!(!root.join("sandboxes").exists());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn evolve_planned_cli_destroys_sandbox_and_updates_metrics() {
    let root = temp_crate("phase37-evolve-planned");
    seed_graph(&root);

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
    let sandbox_entries = fs::read_dir(root.join("sandboxes"))
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(sandbox_entries, 0);
    let metrics = fs::read_to_string(root.join("memory/metrics.json")).expect("metrics");
    assert!(metrics.contains("\"total_runs\": 1"));
    assert!(metrics.contains("\"last_run_id\""));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn repo_ingestion_ast_path_is_read_only() {
    let root = temp_crate("phase37-ingest");
    let before = fs::read_to_string(root.join("src/probe.rs")).expect("read before");

    ingest_repo_patterns(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("ingest");

    let after = fs::read_to_string(root.join("src/probe.rs")).expect("read after");
    assert_eq!(before, after);
    let graph = fs::read_to_string(root.join("memory/graph.json")).expect("graph");
    assert!(graph.contains("pattern:struct:Probe"));
    assert!(graph.contains("pattern:enum:ProbeMode"));

    fs::remove_dir_all(root).expect("cleanup");
}

fn plan(id: &str, expected_gain: f32, estimated_risk: f32, evidence_weight: f32) -> MutationPlan {
    MutationPlan {
        id: id.to_string(),
        objective: MutationObjective::ImproveReliability,
        target_file: "src/evolution/validator.rs".to_string(),
        mutation_kind: MutationKind::AppendComment,
        reason: "test ranking".to_string(),
        expected_gain,
        estimated_risk,
        evidence_weight,
        graph_evidence: Vec::new(),
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

fn temp_crate(name: &str) -> PathBuf {
    let root = temp_dir(name);
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"phase37_temp\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write cargo");
    fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("write main");
    fs::write(
        root.join("src/probe.rs"),
        "pub struct Probe;\npub enum ProbeMode { A }\npub fn probe() {}\n",
    )
    .expect("write probe");
    root
}

fn temp_dir(name: &str) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_millis();
    std::env::temp_dir().join(format!("{name}-{}-{millis}", std::process::id()))
}
