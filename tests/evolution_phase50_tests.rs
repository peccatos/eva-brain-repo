use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use eva_runtime_with_task_validator::{
    generate_from_recombined_hypothesis, load_recombined_hypotheses, validate_mutation,
};

#[test]
fn recombination_avoids_risky_target_file() {
    let root = temp_crate("phase50-risky-target");
    seed_recombination_memory(&root, true, false);

    let hypotheses =
        load_recombined_hypotheses(root.join("memory").to_str().unwrap()).expect("hypotheses");
    assert!(!hypotheses.is_empty());
    assert!(hypotheses
        .iter()
        .all(|hypothesis| { hypothesis.suggested_target != "src/runtime_cycle.rs" }));
    assert!(hypotheses.iter().any(|hypothesis| {
        hypothesis.suggested_target == "tests/evolution_generated_tests.rs"
            && hypothesis
                .avoided_risks
                .iter()
                .any(|value| value.contains("src/runtime_cycle.rs"))
    }));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn recombination_prefers_successful_add_unit_test_pattern() {
    let root = temp_crate("phase50-prefer-addunittest");
    seed_recombination_memory(&root, false, false);

    let hypotheses =
        load_recombined_hypotheses(root.join("memory").to_str().unwrap()).expect("hypotheses");
    assert!(!hypotheses.is_empty());
    assert_eq!(hypotheses[0].suggested_mutation_kind, "addunittest");

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn recombination_output_is_deterministic() {
    let root = temp_crate("phase50-deterministic");
    seed_recombination_memory(&root, true, false);

    let first =
        load_recombined_hypotheses(root.join("memory").to_str().unwrap()).expect("first load");
    let second =
        load_recombined_hypotheses(root.join("memory").to_str().unwrap()).expect("second load");
    assert_eq!(first, second);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn recombine_patterns_creates_no_sandbox() {
    let root = temp_crate("phase50-no-sandbox");
    seed_recombination_memory(&root, true, false);

    let output = run_ok(&root, &["--recombine-patterns"]);
    assert!(output.contains("recombined:"));
    assert!(fs::read_dir(root.join("sandboxes"))
        .expect("sandboxes")
        .next()
        .is_none());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn evolve_recombined_creates_candidate_or_safe_non_candidate_without_mutating_core() {
    let root = temp_crate("phase50-evolve");
    seed_recombination_memory(&root, true, false);
    let main_before = fs::read_to_string(root.join("src/main.rs")).expect("main before");

    let output = run_ok(&root, &["--evolve-recombined"]);
    assert!(output.contains("evolve_recombined_status: ok"));

    let log = fs::read_to_string(root.join("memory/evolution.jsonl")).expect("evolution log");
    assert!(log.contains("\"hypothesis_id\""));
    assert_eq!(
        fs::read_to_string(root.join("src/main.rs")).expect("main after"),
        main_before
    );

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn generated_recombined_mutation_passes_validator() {
    let root = temp_crate("phase50-validator");
    seed_recombination_memory(&root, true, false);

    let hypothesis = load_recombined_hypotheses(root.join("memory").to_str().unwrap())
        .expect("hypotheses")
        .into_iter()
        .next()
        .expect("top hypothesis");
    let mutation =
        generate_from_recombined_hypothesis(&hypothesis).expect("generate recombined mutation");
    validate_mutation(&mutation).expect("validated mutation");

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn reports_include_recombination_fields() {
    let root = temp_crate("phase50-report-fields");
    seed_recombination_memory(&root, true, false);

    run_ok(&root, &["--evolve-recombined"]);
    let report_dir = root.join("memory/reports");
    let report_json = latest_file_with_suffix(&report_dir, ".report.json");
    let report_md = latest_file_with_suffix(&report_dir, ".ru.md");
    let json = fs::read_to_string(report_json).expect("json report");
    let md = fs::read_to_string(report_md).expect("md report");
    assert!(json.contains("\"hypothesis_id\""));
    assert!(json.contains("\"source_patterns\""));
    assert!(md.contains("## Рекомбинация"));
    assert!(md.contains("Avoided risks:"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn forbidden_targets_are_never_selected() {
    let root = temp_crate("phase50-forbidden");
    seed_recombination_memory(&root, false, true);

    let hypotheses =
        load_recombined_hypotheses(root.join("memory").to_str().unwrap()).expect("hypotheses");
    assert!(!hypotheses.is_empty());
    assert!(hypotheses.iter().all(|hypothesis| {
        hypothesis.suggested_target != "src/main.rs"
            && hypothesis.suggested_target != "src/lib.rs"
            && hypothesis.suggested_target != "Cargo.toml"
            && !hypothesis.suggested_target.starts_with("src/core/")
    }));

    fs::remove_dir_all(root).expect("cleanup");
}

fn temp_crate(name: &str) -> PathBuf {
    let root = temp_dir(name);
    fs::create_dir_all(root.join("src/core")).expect("src/core");
    fs::create_dir_all(root.join("src/evolution")).expect("src/evolution");
    fs::create_dir_all(root.join("memory/patterns")).expect("memory patterns");
    fs::create_dir_all(root.join("sandboxes")).expect("sandboxes");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"phase50_temp\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("cargo toml");
    fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("main");
    fs::write(root.join("src/lib.rs"), "pub fn library_probe() {}\n").expect("lib");
    fs::write(root.join("src/core/locked.rs"), "pub fn locked() {}\n").expect("core");
    fs::write(root.join("src/runtime_cycle.rs"), "pub fn cycle() {}\n").expect("runtime");
    fs::write(
        root.join("src/validator_probe.rs"),
        "pub fn validator_probe() -> bool { true }\n",
    )
    .expect("validator");
    fs::write(
        root.join("src/evolution/metrics.rs"),
        "pub fn metrics_probe() -> usize { 1 }\n",
    )
    .expect("metrics");
    root
}

fn seed_recombination_memory(root: &PathBuf, risky_runtime: bool, include_forbidden: bool) {
    let graph = if include_forbidden {
        r#"{
  "nodes": [
    {"id":"file:src/runtime_cycle.rs","kind":"File"},
    {"id":"file:src/validator_probe.rs","kind":"File"},
    {"id":"file:src/main.rs","kind":"File"},
    {"id":"file:src/lib.rs","kind":"File"},
    {"id":"file:src/core/locked.rs","kind":"File"},
    {"id":"file:tests/evolution_generated_tests.rs","kind":"File"},
    {"id":"pattern:function:runtime_cycle","kind":"Pattern"},
    {"id":"pattern:function:validator_probe","kind":"Pattern"}
  ],
  "edges": [
    {"from":"pattern:function:runtime_cycle","to":"file:src/runtime_cycle.rs","relation":"found_in"},
    {"from":"pattern:function:validator_probe","to":"file:src/validator_probe.rs","relation":"found_in"}
  ]
}"#
    } else {
        r#"{
  "nodes": [
    {"id":"file:src/runtime_cycle.rs","kind":"File"},
    {"id":"file:src/validator_probe.rs","kind":"File"},
    {"id":"file:tests/evolution_generated_tests.rs","kind":"File"},
    {"id":"pattern:function:runtime_cycle","kind":"Pattern"},
    {"id":"pattern:function:validator_probe","kind":"Pattern"}
  ],
  "edges": [
    {"from":"pattern:function:runtime_cycle","to":"file:src/runtime_cycle.rs","relation":"found_in"},
    {"from":"pattern:function:validator_probe","to":"file:src/validator_probe.rs","relation":"found_in"}
  ]
}"#
    };
    fs::write(root.join("memory/graph.json"), graph).expect("graph");

    fs::write(
        root.join("memory/success_patterns.json"),
        r#"[
  {
    "pattern_id":"plan:test",
    "target_area":"tests",
    "target_file":"tests/evolution_generated_tests.rs",
    "mutation_kind":"addunittest",
    "success_count":4,
    "average_score":9.5,
    "bonus":1.0,
    "last_run_id":"seed-success"
  }
]"#,
    )
    .expect("success patterns");

    fs::write(
        root.join("memory/patterns/local_distilled_patterns.json"),
        r#"{
  "generated_at": 1,
  "top_successful_mutation_kinds": [
    {"key":"addunittest","count":4,"average_score":9.5}
  ],
  "risky_target_files": [
    {"target_file":"src/runtime_cycle.rs","fail_count":2,"penalty":0.8}
  ],
  "preferred_objectives": [
    {"key":"ImproveValidation","count":3,"average_score":8.0},
    {"key":"ImproveReliability","count":2,"average_score":7.0}
  ]
}"#,
    )
    .expect("distilled");

    let regressions = if risky_runtime {
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
]"#
    } else {
        "[]"
    };
    fs::write(root.join("memory/regressions.json"), regressions).expect("regressions");

    fs::write(
        root.join("memory/metrics.json"),
        r#"{
  "total_runs": 12,
  "passed_runs": 12,
  "failed_runs": 0,
  "candidate_count": 6,
  "replay_passed": 2,
  "promoted_count": 2,
  "average_score": 8.4,
  "last_run_id": "seed-last"
}"#,
    )
    .expect("metrics");

    fs::write(
        root.join("memory/evolution.jsonl"),
        concat!(
            "{\"run_id\":\"seed-1\",\"plan_id\":\"plan:test\",\"hypothesis_id\":null,\"objective\":\"ImproveValidation\",\"graph_evidence\":[],\"mutation_id\":\"seed-1\",\"mutation_digest\":\"\",\"status\":\"candidate\",\"target_file\":\"tests/evolution_generated_tests.rs\",\"mutation_kind\":\"addunittest\",\"risk\":0.1,\"score\":9.0,\"useful_change\":true,\"duplicate_rejected\":false,\"regression_penalty\":0.0,\"success_bonus\":0.5,\"cargo_check_ok\":true,\"cargo_test_ok\":true,\"cargo_run_ok\":true,\"retained_in_core\":false,\"sandbox_destroyed\":true,\"stdout_digest\":\"\",\"stderr_digest\":\"\",\"stderr_tail\":\"\",\"timestamp_unix\":1}\n",
            "{\"run_id\":\"seed-2\",\"plan_id\":\"plan:test-2\",\"hypothesis_id\":null,\"objective\":\"ImproveReliability\",\"graph_evidence\":[],\"mutation_id\":\"seed-2\",\"mutation_digest\":\"\",\"status\":\"candidate\",\"target_file\":\"tests/evolution_generated_tests.rs\",\"mutation_kind\":\"addunittest\",\"risk\":0.1,\"score\":8.0,\"useful_change\":true,\"duplicate_rejected\":false,\"regression_penalty\":0.0,\"success_bonus\":0.5,\"cargo_check_ok\":true,\"cargo_test_ok\":true,\"cargo_run_ok\":true,\"retained_in_core\":false,\"sandbox_destroyed\":true,\"stdout_digest\":\"\",\"stderr_digest\":\"\",\"stderr_tail\":\"\",\"timestamp_unix\":2}\n"
        ),
    )
    .expect("evolution log");
}

fn latest_file_with_suffix(dir: &PathBuf, suffix: &str) -> PathBuf {
    let mut entries = fs::read_dir(dir)
        .expect("report dir")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.to_string_lossy().ends_with(suffix))
        .collect::<Vec<_>>();
    entries.sort();
    entries.pop().expect("latest file")
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
