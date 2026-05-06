use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use eva_runtime_with_task_validator::{
    load_recombined_hypotheses, normalized_generated_test_name, MutationKind, MutationObjective,
    MutationPlan,
};

#[test]
fn addunittest_saturation_lowers_recombination_rank() {
    let root = temp_crate("phase51-saturation");
    seed_diversity_memory(&root);

    let hypotheses =
        load_recombined_hypotheses(root.join("memory").to_str().unwrap()).expect("hypotheses");
    assert!(!hypotheses.is_empty());
    assert_ne!(hypotheses[0].suggested_mutation_kind, "addunittest");
    assert!(hypotheses
        .iter()
        .any(
            |hypothesis| hypothesis.suggested_mutation_kind == "addunittest"
                && hypothesis.saturation_penalty > 0.0
        ));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn underexplored_add_replay_assertion_gets_diversity_bonus() {
    let root = temp_crate("phase51-diversity-bonus");
    seed_diversity_memory(&root);

    let replay = load_recombined_hypotheses(root.join("memory").to_str().unwrap())
        .expect("hypotheses")
        .into_iter()
        .find(|hypothesis| hypothesis.suggested_mutation_kind == "addreplayassertion")
        .expect("replay hypothesis");
    assert!(replay.diversity_bonus > 0.0);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn repeated_target_gets_penalty() {
    let root = temp_crate("phase51-target-penalty");
    seed_diversity_memory(&root);

    let hypotheses =
        load_recombined_hypotheses(root.join("memory").to_str().unwrap()).expect("hypotheses");
    assert!(hypotheses.iter().any(|hypothesis| {
        hypothesis.suggested_target == "tests/evolution_generated_tests.rs"
            && hypothesis.repeated_target_penalty > 0.0
    }));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn unsafe_mutation_kinds_are_never_selected() {
    let root = temp_crate("phase51-unsafe-kinds");
    seed_diversity_memory(&root);

    let hypotheses =
        load_recombined_hypotheses(root.join("memory").to_str().unwrap()).expect("hypotheses");
    assert!(hypotheses.iter().all(|hypothesis| {
        !matches!(
            hypothesis.suggested_mutation_kind.as_str(),
            "appendcomment" | "deletecode" | "rewritefunction" | "freediff" | "dependencyadd"
        )
    }));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn portfolio_cli_prints_summary() {
    let root = temp_crate("phase51-portfolio-cli");
    seed_diversity_memory(&root);

    let output = run_ok(&root, &["--portfolio"]);
    assert!(output.contains("addunittest"));
    assert!(output.contains("saturation="));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn recombination_remains_deterministic_under_portfolio_pressure() {
    let root = temp_crate("phase51-deterministic");
    seed_diversity_memory(&root);

    let first =
        load_recombined_hypotheses(root.join("memory").to_str().unwrap()).expect("first load");
    let second =
        load_recombined_hypotheses(root.join("memory").to_str().unwrap()).expect("second load");
    assert_eq!(first, second);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn generated_test_names_are_short_valid_and_deterministic() {
    let plan = MutationPlan {
        id: "recombined:src/evolution/memory.rs:addunittest:tests/evolution_generated_tests.rs"
            .to_string(),
        objective: MutationObjective::ImproveReliability,
        target_file: "tests/evolution_generated_tests.rs".to_string(),
        mutation_kind: MutationKind::AddUnitTest,
        reason: "phase51 test".to_string(),
        expected_gain: 0.5,
        estimated_risk: 0.1,
        evidence_weight: 0.2,
        graph_evidence: vec!["pattern:function:append_jsonl".to_string()],
    };
    let first = normalized_generated_test_name(&plan, "deterministic");
    let second = normalized_generated_test_name(&plan, "deterministic");

    assert_eq!(first, second);
    assert!(first.len() <= 80, "name too long: {}", first.len());
    assert!(is_valid_rust_identifier(&first), "{first}");
}

fn is_valid_rust_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn seed_diversity_memory(root: &PathBuf) {
    fs::create_dir_all(root.join("memory/patterns")).expect("patterns");
    fs::create_dir_all(root.join("memory/candidates")).expect("candidates");
    fs::write(
        root.join("memory/graph.json"),
        r#"{
  "nodes": [
    {"id":"file:src/runtime_cycle.rs","kind":"File"},
    {"id":"file:src/evolution/metrics.rs","kind":"File"},
    {"id":"file:src/validator_probe.rs","kind":"File"},
    {"id":"pattern:function:runtime_cycle","kind":"Pattern"},
    {"id":"pattern:function:metrics_probe","kind":"Pattern"},
    {"id":"pattern:function:validator_probe","kind":"Pattern"}
  ],
  "edges": [
    {"from":"pattern:function:runtime_cycle","to":"file:src/runtime_cycle.rs","relation":"found_in"},
    {"from":"pattern:function:metrics_probe","to":"file:src/evolution/metrics.rs","relation":"found_in"},
    {"from":"pattern:function:validator_probe","to":"file:src/validator_probe.rs","relation":"found_in"}
  ]
}"#,
    )
    .expect("graph");
    fs::write(
        root.join("memory/success_patterns.json"),
        r#"[
  {
    "pattern_id":"plan:addunittest",
    "target_area":"tests",
    "target_file":"tests/evolution_generated_tests.rs",
    "mutation_kind":"addunittest",
    "success_count":12,
    "average_score":9.8,
    "bonus":1.0,
    "last_run_id":"seed-addunittest"
  }
]"#,
    )
    .expect("success patterns");
    fs::write(
        root.join("memory/regressions.json"),
        r#"[
  {
    "pattern_id":"pattern:appendcomment:src/runtime_cycle.rs",
    "target_area":"src",
    "target_file":"src/runtime_cycle.rs",
    "mutation_kind":"appendcomment",
    "failure_status":"non_useful",
    "fail_count":2,
    "penalty":0.8,
    "last_run_id":"seed-regression"
  }
]"#,
    )
    .expect("regressions");
    fs::write(
        root.join("memory/patterns/local_distilled_patterns.json"),
        r#"{
  "generated_at": 1,
  "top_successful_mutation_kinds": [
    {"key":"addunittest","count":12,"average_score":9.8}
  ],
  "risky_target_files": [
    {"target_file":"src/runtime_cycle.rs","fail_count":2,"penalty":0.8}
  ],
  "preferred_objectives": [
    {"key":"ImproveReliability","count":4,"average_score":8.0},
    {"key":"ImproveGraphMemory","count":2,"average_score":7.5}
  ]
}"#,
    )
    .expect("distilled patterns");
    fs::write(
        root.join("memory/metrics.json"),
        r#"{
  "total_runs": 20,
  "passed_runs": 20,
  "failed_runs": 0,
  "candidate_count": 10,
  "replay_passed": 3,
  "promoted_count": 3,
  "average_score": 8.2,
  "last_run_id": "seed-last"
}"#,
    )
    .expect("metrics");
    fs::write(
        root.join("memory/portfolio.json"),
        r#"{
  "kinds": [
    {
      "mutation_kind":"addunittest",
      "seen_count":14,
      "success_count":14,
      "candidate_count":10,
      "replay_passed_count":3,
      "promoted_count":3,
      "average_score":9.2,
      "saturation_score":0.30,
      "last_used_at":10
    },
    {
      "mutation_kind":"addreplayassertion",
      "seen_count":0,
      "success_count":0,
      "candidate_count":0,
      "replay_passed_count":0,
      "promoted_count":0,
      "average_score":0.0,
      "saturation_score":0.0,
      "last_used_at":0
    },
    {
      "mutation_kind":"addmetricupdate",
      "seen_count":1,
      "success_count":1,
      "candidate_count":0,
      "replay_passed_count":0,
      "promoted_count":0,
      "average_score":7.0,
      "saturation_score":0.0,
      "last_used_at":9
    }
  ]
}"#,
    )
    .expect("portfolio");

    for index in 0..5 {
        fs::write(
            root.join("memory/candidates")
                .join(format!("seed-{index}.summary.json")),
            format!(
                "{{\"run_id\":\"seed-{index}\",\"mutation_id\":\"seed-{index}\",\"mutation_digest\":\"d{index}\",\"status\":\"candidate\",\"target_file\":\"tests/evolution_generated_tests.rs\",\"mutation_kind\":\"addunittest\",\"risk\":0.1,\"score\":9.0,\"useful_change\":true,\"non_candidate_reason\":null,\"duplicate_rejected\":false,\"regression_penalty\":0.0,\"success_bonus\":0.0,\"cargo_check_ok\":true,\"cargo_test_ok\":true,\"cargo_run_ok\":true,\"stdout_digest\":\"\",\"stderr_digest\":\"\",\"stderr_tail\":\"\",\"timestamp_unix\":{}}}",
                index + 1
            ),
        )
        .expect("candidate summary");
    }
}

fn temp_crate(name: &str) -> PathBuf {
    let root = temp_dir(name);
    fs::create_dir_all(root.join("src/evolution")).expect("src evolution");
    fs::create_dir_all(root.join("memory")).expect("memory");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"phase51_temp\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("cargo");
    fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("main");
    fs::write(root.join("src/runtime_cycle.rs"), "pub fn cycle() {}\n").expect("runtime");
    fs::write(
        root.join("src/evolution/metrics.rs"),
        "pub fn metrics_probe() -> usize { 1 }\n",
    )
    .expect("metrics");
    fs::write(
        root.join("src/validator_probe.rs"),
        "pub fn validator_probe() -> bool { true }\n",
    )
    .expect("validator");
    root
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

fn temp_dir(name: &str) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_millis();
    std::env::temp_dir().join(format!("{name}-{}-{millis}", std::process::id()))
}
