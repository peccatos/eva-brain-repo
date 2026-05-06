use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use eva_runtime_with_task_validator::contracts::{MutationContract, MutationKind};
use eva_runtime_with_task_validator::{load_report_json, validate_mutation};

#[test]
fn add_unit_test_validates_tests_target() {
    let mutation = MutationContract {
        id: "unit-test-ok".to_string(),
        kind: MutationKind::AddUnitTest,
        target_file: "tests/evolution_generated_tests.rs".to_string(),
        search: None,
        replace: None,
        append: Some("#[test]\nfn generated_test() { assert_eq!(2 + 2, 4); }\n".to_string()),
        reason: "bounded generated test".to_string(),
        expected_gain: 0.5,
        risk: 0.1,
    };
    validate_mutation(&mutation).expect("generated unit test should validate");
}

#[test]
fn add_unit_test_rejects_non_tests_target() {
    let mutation = MutationContract {
        id: "unit-test-bad".to_string(),
        kind: MutationKind::AddUnitTest,
        target_file: "src/evolution/metrics.rs".to_string(),
        search: None,
        replace: None,
        append: Some("#[test]\nfn generated_test() { assert!(true); }\n".to_string()),
        reason: "invalid target".to_string(),
        expected_gain: 0.5,
        risk: 0.1,
    };
    let error = validate_mutation(&mutation).expect_err("non-tests target must reject");
    assert!(error.contains("tests/"));
}

#[test]
fn add_replay_assertion_validates_tests_target() {
    let mutation = MutationContract {
        id: "replay-ok".to_string(),
        kind: MutationKind::AddReplayAssertion,
        target_file: "tests/evolution_generated_tests.rs".to_string(),
        search: None,
        replace: None,
        append: Some(
            "#[test]\nfn replay_assertion() { let fixture = [true, true, false]; assert_eq!(fixture.iter().filter(|ok| **ok).count(), 2); }\n"
                .to_string(),
        ),
        reason: "bounded replay assertion".to_string(),
        expected_gain: 0.55,
        risk: 0.1,
    };
    validate_mutation(&mutation).expect("generated replay assertion should validate");
}

#[test]
fn append_comment_still_cannot_be_candidate() {
    let root = temp_crate("phase45r-evolve");
    let output = Command::new(env!("CARGO_BIN_EXE_eva_runtime_with_task_validator"))
        .arg("--evolve")
        .current_dir(&root)
        .output()
        .expect("run evolve");
    assert!(output.status.success());
    let candidate_count = summary_count(root.join("memory/candidates"));
    assert_eq!(candidate_count, 0);
    let reports = report_files(root.join("memory/reports"));
    assert_eq!(
        reports
            .iter()
            .filter(|path| path.to_string_lossy().ends_with(".ru.md"))
            .count(),
        1
    );
    assert_eq!(sandbox_entries(&root), 0);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn useful_template_can_become_candidate() {
    let root = temp_crate("phase45r-planned");
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

    let summaries = fs::read_dir(root.join("memory/candidates"))
        .expect("candidate dir")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    assert!(!summaries.is_empty());

    let latest_summary = summaries
        .iter()
        .find(|path| path.to_string_lossy().contains(".summary.json"))
        .expect("summary file");
    let summary = fs::read_to_string(latest_summary).expect("read summary");
    assert!(summary.contains("\"mutation_kind\": \"addunittest\""));
    assert!(summary.contains("\"useful_change\": true"));
    assert_eq!(sandbox_entries(&root), 0);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn russian_report_is_written_and_printed() {
    let root = temp_crate("phase45r-report");
    seed_graph(&root);

    let output = Command::new(env!("CARGO_BIN_EXE_eva_runtime_with_task_validator"))
        .arg("--evolve-planned")
        .current_dir(&root)
        .output()
        .expect("run planned evolution");
    assert!(output.status.success());

    let run_id = latest_run_id(&root);
    let last_report = Command::new(env!("CARGO_BIN_EXE_eva_runtime_with_task_validator"))
        .arg("--last-report")
        .current_dir(&root)
        .output()
        .expect("print last report");
    assert!(last_report.status.success());
    let last_report_stdout = String::from_utf8_lossy(&last_report.stdout);
    assert!(last_report_stdout.contains("Отчёт EVA"));
    assert!(last_report_stdout.contains("Цель"));

    let specific_report = Command::new(env!("CARGO_BIN_EXE_eva_runtime_with_task_validator"))
        .args(["--report", &run_id])
        .current_dir(&root)
        .output()
        .expect("print report");
    assert!(specific_report.status.success());
    let report_stdout = String::from_utf8_lossy(&specific_report.stdout);
    assert!(report_stdout.contains("Риск"));
    assert!(report_stdout.contains("Следующий шаг"));

    let json =
        load_report_json(root.join("memory").to_str().unwrap(), &run_id).expect("load report json");
    assert_eq!(json.run_id, run_id);
    assert_eq!(json.target_file, "tests/evolution_generated_tests.rs");

    fs::remove_dir_all(root).expect("cleanup");
}

fn temp_crate(name: &str) -> PathBuf {
    let root = temp_dir(name);
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::create_dir_all(root.join("memory")).expect("create memory");
    fs::create_dir_all(root.join("sandboxes")).expect("create sandboxes");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"phase45r_temp\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write cargo");
    fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("write main");
    fs::write(root.join("src/probe.rs"), "pub fn probe() {}\n").expect("write probe");
    fs::write(root.join("src/runtime_cycle.rs"), "pub fn cycle() {}\n")
        .expect("write runtime cycle");
    root
}

fn seed_graph(root: &PathBuf) {
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

fn summary_count(path: PathBuf) -> usize {
    fs::read_dir(path)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| entry.path().to_string_lossy().contains(".summary.json"))
                .count()
        })
        .unwrap_or(0)
}

fn report_files(path: PathBuf) -> Vec<PathBuf> {
    fs::read_dir(path)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .collect()
        })
        .unwrap_or_default()
}

fn latest_run_id(root: &PathBuf) -> String {
    let mut reports = report_files(root.join("memory/reports"));
    reports.sort();
    reports
        .last()
        .and_then(|path| path.file_stem())
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.trim_end_matches(".ru").to_string())
        .expect("latest run id")
}

fn sandbox_entries(root: &PathBuf) -> usize {
    fs::read_dir(root.join("sandboxes"))
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| entry.file_name() != ".gitkeep")
                .count()
        })
        .unwrap_or(0)
}

fn temp_dir(name: &str) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_millis();
    std::env::temp_dir().join(format!("{name}-{}-{millis}", std::process::id()))
}
