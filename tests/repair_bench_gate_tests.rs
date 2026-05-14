use std::path::PathBuf;
use std::process::Command;

#[path = "evolution_test_support.rs"]
mod evolution_test_support;

use eva_runtime_with_task_validator::{
    benchmark_from_report, detect_repair_bench_regressions, print_repair_bench_gate,
    print_repair_bench_history, run_repair_bench, run_repair_bench_gate, run_repair_bench_history,
    RepairBenchBaseline, RepairBenchCaseResult, RepairBenchCaseStatus, RepairBenchGateRequest,
    RepairBenchGateStatus, RepairBenchMetricSummary, RepairBenchReport, RepairBenchRequest,
    RepairBenchStatus,
};

#[test]
fn repair_bench_history_handles_empty_history() {
    let root = evolution_test_support::unique_evolution_root("repair-bench-history-empty");
    let output_dir = root.join("bench-output");
    let report = run_repair_bench_history(output_dir.clone()).expect("history report");
    assert_eq!(report.runs, 0);
    assert!(report.latest.is_none());
    let rendered = print_repair_bench_history(output_dir, false, None).expect("history text");
    assert!(rendered.contains("Runs: 0"));
    assert!(rendered.contains("Status: empty"));
    evolution_test_support::remove_root(&root);
}

#[test]
fn repair_bench_history_records_run() {
    let root = evolution_test_support::unique_evolution_root("repair-bench-history-run");
    let output_dir = root.join("bench-output");
    let _ = run_repair_bench(RepairBenchRequest {
        bench_id: "repair-bench-history-run".to_string(),
        suite: "phase21".to_string(),
        output_dir: output_dir.clone(),
        no_llm: true,
        json: false,
    })
    .expect("bench report");
    let report = run_repair_bench_history(output_dir.clone()).expect("history report");
    assert_eq!(report.runs, 1);
    let latest = report.latest.expect("latest");
    assert_eq!(latest.suite, "phase21");
    assert_eq!(latest.passed_cases, 4);
    assert_eq!(latest.partial_cases, 1);
    assert_eq!(latest.failed_cases, 0);
    assert!(output_dir.join("history.jsonl").exists());
    assert!(output_dir.join("latest.json").exists());
    evolution_test_support::remove_root(&root);
}

#[test]
fn repair_bench_gate_passes_against_phase21_baseline() {
    let root = evolution_test_support::unique_evolution_root("repair-bench-gate-pass");
    let output_dir = root.join("bench-output");
    let report = run_repair_bench_gate(RepairBenchGateRequest {
        suite: "phase21".to_string(),
        baseline: "latest".to_string(),
        baseline_file: None,
        output_dir: output_dir.clone(),
        json: false,
    })
    .expect("gate report");
    assert!(matches!(report.status, RepairBenchGateStatus::Passed));
    assert!(report.regressions.is_empty());
    assert_eq!(report.current_report.passed_cases, 4);
    assert!(report.output_dir.join("report.json").exists());
    assert!(report.output_dir.join("report.md").exists());
    evolution_test_support::remove_root(&root);
}

#[test]
fn repair_bench_gate_fails_when_failed_cases_increase() {
    let baseline = RepairBenchBaseline {
        suite: "phase21".to_string(),
        total_cases: 5,
        actionable_cases: 4,
        passed_cases: 4,
        partial_cases: 1,
        failed_cases: 0,
        detection_success_rate: 1.0,
        repair_success_rate: 1.0,
        validation_success_rate: 1.0,
        evidence_success_rate: 1.0,
    };
    let current_report = fake_report(4, 1, 0);
    let current = benchmark_from_report(&current_report);
    let regressions = detect_repair_bench_regressions(&baseline, &current, &current_report);
    assert!(regressions
        .iter()
        .any(|regression| regression.field == "failed_cases increased"));
}

#[test]
fn repair_bench_gate_fails_when_passed_cases_decrease() {
    let baseline = RepairBenchBaseline {
        suite: "phase21".to_string(),
        total_cases: 5,
        actionable_cases: 4,
        passed_cases: 4,
        partial_cases: 1,
        failed_cases: 0,
        detection_success_rate: 1.0,
        repair_success_rate: 1.0,
        validation_success_rate: 1.0,
        evidence_success_rate: 1.0,
    };
    let current_report = fake_report(3, 1, 1);
    let current = benchmark_from_report(&current_report);
    let regressions = detect_repair_bench_regressions(&baseline, &current, &current_report);
    assert!(regressions
        .iter()
        .any(|regression| regression.field == "passed_cases decreased"));
}

#[test]
fn repair_bench_gate_ignores_unknown_empty_project_partial() {
    let root = evolution_test_support::unique_evolution_root("repair-bench-gate-partial");
    let output_dir = root.join("bench-output");
    let report = run_repair_bench_gate(RepairBenchGateRequest {
        suite: "phase21".to_string(),
        baseline: "latest".to_string(),
        baseline_file: None,
        output_dir,
        json: false,
    })
    .expect("gate report");
    assert!(matches!(report.status, RepairBenchGateStatus::Passed));
    assert_eq!(report.current_report.partial_cases, 1);
    assert_eq!(report.current_report.failed_cases, 0);
    evolution_test_support::remove_root(&root);
}

#[test]
fn repair_bench_gate_json_output_is_parseable() {
    let root = evolution_test_support::unique_evolution_root("repair-bench-gate-json");
    let output_dir = root.join("bench-output");
    let output = print_repair_bench_gate(RepairBenchGateRequest {
        suite: "phase21".to_string(),
        baseline: "latest".to_string(),
        baseline_file: None,
        output_dir,
        json: true,
    })
    .expect("gate text");
    let report: eva_runtime_with_task_validator::RepairBenchGateReport =
        serde_json::from_str(&output).expect("parse gate json");
    assert_eq!(report.suite, "phase21");
    assert!(matches!(report.status, RepairBenchGateStatus::Passed));
    evolution_test_support::remove_root(&root);
}

#[test]
fn repair_bench_gate_does_not_mutate_source_tree() {
    let root = evolution_test_support::unique_evolution_root("repair-bench-gate-clean");
    let output_dir = root.join("bench-output");
    let before = git_status_short();
    let _ = run_repair_bench_gate(RepairBenchGateRequest {
        suite: "phase21".to_string(),
        baseline: "latest".to_string(),
        baseline_file: None,
        output_dir,
        json: false,
    })
    .expect("gate report");
    let after = git_status_short();
    assert_eq!(before, after);
    evolution_test_support::remove_root(&root);
}

fn fake_report(
    passed_cases: usize,
    failed_cases: usize,
    partial_cases: usize,
) -> RepairBenchReport {
    let total_cases = passed_cases + failed_cases + partial_cases;
    let case_results = vec![
        fake_case(
            "actionable_pass",
            Some("missing_ci"),
            RepairBenchCaseStatus::Passed,
        ),
        fake_case(
            "actionable_fail",
            Some("missing_ci"),
            RepairBenchCaseStatus::Failed,
        ),
        fake_case("unknown", None, RepairBenchCaseStatus::Partial),
    ];
    RepairBenchReport {
        bench_id: "fake-bench".to_string(),
        suite: "phase21".to_string(),
        status: RepairBenchStatus::Warn,
        total_cases,
        passed_cases,
        failed_cases,
        partial_cases,
        case_results,
        metrics: RepairBenchMetricSummary {
            total_cases,
            actionable_cases: 1,
            passed_cases,
            failed_cases,
            partial_cases,
            detection_success_rate: 1.0,
            repair_success_rate: 1.0,
            validation_success_rate: 1.0,
            evidence_success_rate: 1.0,
        },
        output_dir: PathBuf::from("/tmp/fake-bench"),
        warnings: Vec::new(),
        blockers: Vec::new(),
    }
}

fn fake_case(
    case_id: &str,
    expected_problem: Option<&str>,
    status: RepairBenchCaseStatus,
) -> RepairBenchCaseResult {
    RepairBenchCaseResult {
        case_id: case_id.to_string(),
        kind: case_id.to_string(),
        target_path: PathBuf::from("/tmp/fake"),
        detected_problem: expected_problem.map(str::to_string),
        expected_problem: expected_problem.map(str::to_string),
        fix_status: "validation_passed".to_string(),
        validation_passed: true,
        evidence_dir: Some(PathBuf::from("/tmp/fake/evidence")),
        files_expected: vec!["file".to_string()],
        files_observed: vec!["file".to_string()],
        status,
        notes: Vec::new(),
    }
}

fn git_status_short() -> Vec<String> {
    let output = Command::new("git")
        .args(["status", "--short"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("git status");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}
