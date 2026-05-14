use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

use eva_runtime_with_task_validator::{
    run_repair_bench, RepairBenchCaseStatus, RepairBenchRequest, RepairBenchStatus,
};

static SHARED_REPORT: OnceLock<eva_runtime_with_task_validator::RepairBenchReport> =
    OnceLock::new();

#[test]
fn repair_bench_phase21_runs_all_cases() {
    let report = shared_report();
    assert_eq!(report.suite, "phase21");
    assert_eq!(report.total_cases, 5);
    assert_eq!(report.case_results.len(), 5);
    assert!(matches!(report.status, RepairBenchStatus::Warn));
    assert_eq!(report.passed_cases, 4);
    assert_eq!(report.partial_cases, 1);
    assert_eq!(report.failed_cases, 0);
}

#[test]
fn repair_bench_writes_report_json_and_markdown() {
    let report = shared_report();
    assert!(report.output_dir.exists());
    assert!(report.output_dir.join("request.json").exists());
    assert!(report.output_dir.join("report.json").exists());
    assert!(report.output_dir.join("report.md").exists());
    for case in &report.case_results {
        assert!(report
            .output_dir
            .join("cases")
            .join(&case.case_id)
            .join("result.json")
            .exists());
    }
}

#[test]
fn repair_bench_missing_ci_passes() {
    let case = case_result("missing_ci");
    assert_eq!(case.detected_problem.as_deref(), Some("missing_ci"));
    assert_eq!(case.expected_problem.as_deref(), Some("missing_ci"));
    assert!(matches!(case.status, RepairBenchCaseStatus::Passed));
    assert!(case
        .files_observed
        .iter()
        .any(|file| file == ".github/workflows/rust-ci.yml"));
    assert!(case.validation_passed);
}

#[test]
fn repair_bench_missing_smoke_test_passes() {
    let case = case_result("missing_smoke_test");
    assert_eq!(case.detected_problem.as_deref(), Some("missing_smoke_test"));
    assert!(matches!(case.status, RepairBenchCaseStatus::Passed));
    assert!(case
        .files_observed
        .iter()
        .any(|file| file == "tests/eve_smoke.rs"));
    assert!(case.validation_passed);
}

#[test]
fn repair_bench_readme_validation_passes() {
    let case = case_result("readme_missing_validation");
    assert_eq!(
        case.detected_problem.as_deref(),
        Some("missing_readme_validation")
    );
    assert!(matches!(case.status, RepairBenchCaseStatus::Passed));
    assert!(case.files_observed.iter().any(|file| file == "README.md"));
    assert!(case.validation_passed);
}

#[test]
fn repair_bench_missing_module_passes() {
    let case = case_result("simple_missing_module");
    assert_eq!(
        case.detected_problem.as_deref(),
        Some("cargo_check_failure")
    );
    assert!(matches!(case.status, RepairBenchCaseStatus::Passed));
    assert!(case
        .files_observed
        .iter()
        .any(|file| file == "src/missing_module.rs"));
    assert!(case.validation_passed);
}

#[test]
fn repair_bench_unknown_empty_project_does_not_panic() {
    let case = case_result("unknown_empty_project");
    assert_eq!(case.detected_problem, None);
    assert!(matches!(case.status, RepairBenchCaseStatus::Partial));
    assert!(!case.validation_passed);
}

#[test]
fn repair_bench_json_output_is_parseable() {
    let output = serde_json::to_string_pretty(shared_report()).expect("json report");
    let report: eva_runtime_with_task_validator::RepairBenchReport =
        serde_json::from_str(&output).expect("parse json");
    assert_eq!(report.total_cases, 5);
    assert!(matches!(report.status, RepairBenchStatus::Warn));
}

#[test]
fn repair_bench_does_not_mutate_eve_source_tree() {
    let before = git_status_short();
    let _ = run_repair_bench(unique_request("mutate-check", false)).expect("bench report");
    let after = git_status_short();
    assert_eq!(before, after);
}

#[test]
fn repair_bench_uses_no_llm_by_default() {
    let report = shared_report();
    let request_path = report.output_dir.join("request.json");
    let contents = fs::read_to_string(&request_path).expect("request json");
    let request: RepairBenchRequest = serde_json::from_str(&contents).expect("request parse");
    assert!(request.no_llm);
}

fn shared_report() -> &'static eva_runtime_with_task_validator::RepairBenchReport {
    SHARED_REPORT.get_or_init(|| run_repair_bench(shared_request(false)).expect("bench report"))
}

fn shared_request(json: bool) -> RepairBenchRequest {
    RepairBenchRequest {
        bench_id: "repair-bench-test-phase21".to_string(),
        suite: "phase21".to_string(),
        output_dir: bench_output_root().join("shared"),
        no_llm: true,
        json,
    }
}

fn unique_request(suffix: &str, json: bool) -> RepairBenchRequest {
    RepairBenchRequest {
        bench_id: format!("repair-bench-test-phase21-{suffix}"),
        suite: "phase21".to_string(),
        output_dir: bench_output_root().join(suffix),
        no_llm: true,
        json,
    }
}

fn case_result(case_id: &str) -> eva_runtime_with_task_validator::RepairBenchCaseResult {
    shared_report()
        .case_results
        .iter()
        .find(|case| case.case_id == case_id)
        .cloned()
        .expect("case result")
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

fn bench_output_root() -> PathBuf {
    std::env::temp_dir().join("eve-repair-bench-tests")
}
