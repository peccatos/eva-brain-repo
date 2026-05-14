use crate::agent::fix::run_fix;
use crate::agent::storage::{id, now_unix, save_json_pretty};
use crate::contracts::{
    FixOnly, FixReport, FixRiskCap, FixStatus, RepairBenchBaseline, RepairBenchCase,
    RepairBenchCaseHistoryStatus, RepairBenchCaseResult, RepairBenchCaseStatus,
    RepairBenchEvidencePaths, RepairBenchGateReport, RepairBenchGateRequest, RepairBenchGateStatus,
    RepairBenchHistoryEntry, RepairBenchHistoryReport, RepairBenchMetricSummary,
    RepairBenchRegression, RepairBenchReport, RepairBenchRequest, RepairBenchStatus,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn run_repair_bench(request: RepairBenchRequest) -> Result<RepairBenchReport, String> {
    let evidence = build_evidence_paths(&request);
    save_json_pretty(&evidence.request_json, &request)?;
    let work_root = evidence.output_dir.join("work");
    fs::create_dir_all(&work_root)
        .map_err(|error| format!("create {}: {error}", work_root.display()))?;
    let suite_cases = build_suite_cases(&request.suite, &request.bench_id, &work_root)?;

    let mut case_results = Vec::with_capacity(suite_cases.len());
    for case in suite_cases {
        case_results.push(run_bench_case(&request, &case, &evidence)?);
    }

    let metrics = summarize_metrics(&case_results);
    let total_cases = case_results.len();
    let passed_cases = case_results
        .iter()
        .filter(|case| matches!(case.status, RepairBenchCaseStatus::Passed))
        .count();
    let failed_cases = case_results
        .iter()
        .filter(|case| matches!(case.status, RepairBenchCaseStatus::Failed))
        .count();
    let partial_cases = case_results
        .iter()
        .filter(|case| matches!(case.status, RepairBenchCaseStatus::Partial))
        .count();
    let status = if failed_cases > 0 {
        RepairBenchStatus::Failed
    } else if partial_cases > 0 {
        RepairBenchStatus::Warn
    } else {
        RepairBenchStatus::Passed
    };

    let report = RepairBenchReport {
        bench_id: request.bench_id.clone(),
        suite: request.suite.clone(),
        status,
        total_cases,
        passed_cases,
        failed_cases,
        partial_cases,
        case_results,
        metrics,
        output_dir: evidence.output_dir.clone(),
        warnings: Vec::new(),
        blockers: Vec::new(),
    };

    save_json_pretty(&evidence.report_json, &report)?;
    write_report_markdown(&evidence.report_md, &report)?;
    append_history_entry(&request.output_dir, &report)?;
    Ok(report)
}

pub fn print_repair_bench(request: RepairBenchRequest) -> Result<String, String> {
    let report = run_repair_bench(request.clone())?;
    if request.json {
        serde_json::to_string_pretty(&report).map_err(|error| error.to_string())
    } else {
        Ok(render_report(&report))
    }
}

pub fn run_repair_bench_history(output_dir: PathBuf) -> Result<RepairBenchHistoryReport, String> {
    let paths = history_paths(&output_dir);
    let entries = load_history_entries(&paths.history_jsonl, None)?;
    let runs = entries.len();
    let latest = entries.last().cloned();
    Ok(RepairBenchHistoryReport {
        output_dir,
        history_path: paths.history_jsonl,
        latest_path: paths.latest_json,
        runs,
        entries,
        latest,
        warnings: Vec::new(),
        blockers: Vec::new(),
    })
}

pub fn print_repair_bench_history(
    output_dir: PathBuf,
    json: bool,
    limit: Option<usize>,
) -> Result<String, String> {
    let report = run_repair_bench_history(output_dir)?;
    if json {
        let limited = if let Some(limit) = limit {
            let mut report = report.clone();
            if report.entries.len() > limit {
                report.entries = report.entries[report.entries.len() - limit..].to_vec();
            }
            report.latest = report.entries.last().cloned();
            report
        } else {
            report
        };
        serde_json::to_string_pretty(&limited).map_err(|error| error.to_string())
    } else {
        Ok(render_history_report(&report, limit))
    }
}

pub fn run_repair_bench_gate(
    request: RepairBenchGateRequest,
) -> Result<RepairBenchGateReport, String> {
    let baseline = load_repair_bench_baseline(&request)?;
    let current_request = RepairBenchRequest {
        bench_id: id("repair-bench"),
        suite: request.suite.clone(),
        output_dir: request.output_dir.clone(),
        no_llm: true,
        json: request.json,
    };
    let current_report = run_repair_bench(current_request)?;
    let current = benchmark_from_report(&current_report);
    let regressions = detect_repair_bench_regressions(&baseline, &current, &current_report);
    let status = gate_status_for(&baseline, &current_report, &regressions);
    let gate_output_dir = request
        .output_dir
        .join(&current_report.bench_id)
        .join("gate");
    let report = RepairBenchGateReport {
        bench_id: current_report.bench_id.clone(),
        suite: current_report.suite.clone(),
        status,
        baseline,
        current,
        current_report,
        regressions,
        output_dir: gate_output_dir.clone(),
        warnings: Vec::new(),
        blockers: Vec::new(),
    };
    save_json_pretty(&gate_output_dir.join("report.json"), &report)?;
    write_gate_report_markdown(&gate_output_dir.join("report.md"), &report)?;
    Ok(report)
}

pub fn print_repair_bench_gate(request: RepairBenchGateRequest) -> Result<String, String> {
    let report = run_repair_bench_gate(request.clone())?;
    if request.json {
        serde_json::to_string_pretty(&report).map_err(|error| error.to_string())
    } else {
        Ok(render_gate_report(&report))
    }
}

fn run_bench_case(
    request: &RepairBenchRequest,
    case: &RepairBenchCase,
    evidence: &RepairBenchEvidencePaths,
) -> Result<RepairBenchCaseResult, String> {
    let fix_request = crate::contracts::FixRequest {
        fix_id: id("fix"),
        target_path: case.target_path.clone(),
        dry_run: false,
        apply: case.apply,
        only: case.fix_only.clone(),
        max_files: 3,
        risk_cap: FixRiskCap::Low,
        no_llm: true,
        provider: Some("rule_based".to_string()),
        evidence_dir: PathBuf::from(".eva/fix"),
    };
    let fix_report = run_fix(fix_request)?;
    let fix_status = fix_status_label(&fix_report);
    let validation_passed = matches!(fix_report.status, FixStatus::ValidationPassed);
    let files_observed = observed_files(&case.target_path, &case.expected_files);
    let detected_problem = fix_report.detected_problem.clone();
    let status = classify_case_result(case, &fix_report, &files_observed, validation_passed);
    let mut notes = Vec::new();
    notes.push("direct_api_mode".to_string());
    if request.no_llm {
        notes.push("no_llm:true".to_string());
    }
    if !fix_report.validation_side_effects.is_empty() {
        notes.push(format!(
            "validation_side_effects:{}",
            fix_report.validation_side_effects.join("|")
        ));
    }
    if fix_report.evidence_dir != PathBuf::new() {
        notes.push(format!(
            "evidence_dir:{}",
            fix_report.evidence_dir.display()
        ));
    }

    let result = RepairBenchCaseResult {
        case_id: case.case_id.clone(),
        kind: case.kind.clone(),
        target_path: case.target_path.clone(),
        detected_problem,
        expected_problem: case.expected_problem.clone(),
        fix_status,
        validation_passed,
        evidence_dir: Some(fix_report.evidence_dir.clone()),
        files_expected: case.expected_files.clone(),
        files_observed,
        status,
        notes,
    };

    let case_dir = evidence.cases_dir.join(&case.case_id);
    fs::create_dir_all(&case_dir)
        .map_err(|error| format!("create {}: {error}", case_dir.display()))?;
    save_json_pretty(&case_dir.join("result.json"), &result)?;
    Ok(result)
}

fn classify_case_result(
    case: &RepairBenchCase,
    fix_report: &FixReport,
    observed_files: &[String],
    validation_passed: bool,
) -> RepairBenchCaseStatus {
    let expected_files_ok = case
        .expected_files
        .iter()
        .all(|expected| observed_files.iter().any(|observed| observed == expected));
    let detected_ok = match &case.expected_problem {
        Some(expected) => fix_report.detected_problem.as_deref() == Some(expected.as_str()),
        None => fix_report.detected_problem.is_none(),
    };
    let evidence_ok =
        fix_report.evidence_written && !fix_report.evidence_dir.as_os_str().is_empty();

    if case.expected_problem.is_none() {
        if matches!(
            fix_report.status,
            FixStatus::NoActionableProblem | FixStatus::Blocked
        ) && !fix_report.source_mutation
            && evidence_ok
        {
            return RepairBenchCaseStatus::Partial;
        }
        return RepairBenchCaseStatus::Failed;
    }

    if detected_ok && expected_files_ok && validation_passed && evidence_ok {
        RepairBenchCaseStatus::Passed
    } else if detected_ok || expected_files_ok || validation_passed || evidence_ok {
        RepairBenchCaseStatus::Partial
    } else {
        RepairBenchCaseStatus::Failed
    }
}

fn build_suite_cases(
    suite: &str,
    bench_id: &str,
    work_root: &Path,
) -> Result<Vec<RepairBenchCase>, String> {
    match suite {
        "phase21" => Ok(vec![
            build_missing_ci_case(bench_id, work_root)?,
            build_missing_smoke_case(bench_id, work_root)?,
            build_readme_case(bench_id, work_root)?,
            build_missing_module_case(bench_id, work_root)?,
            build_unknown_empty_case(bench_id, work_root)?,
        ]),
        "phase24x" => Ok(vec![
            build_missing_ci_case(bench_id, work_root)?,
            build_missing_smoke_case(bench_id, work_root)?,
            build_readme_case(bench_id, work_root)?,
            build_missing_module_case(bench_id, work_root)?,
            build_unknown_empty_case(bench_id, work_root)?,
            build_missing_gitignore_case(bench_id, work_root)?,
            build_missing_clippy_case(bench_id, work_root)?,
            build_missing_readme_usage_case(bench_id, work_root)?,
        ]),
        other => Err(format!("unsupported repair bench suite: {other}")),
    }
}

fn build_missing_ci_case(bench_id: &str, work_root: &Path) -> Result<RepairBenchCase, String> {
    let target = create_case_root(work_root, bench_id, "missing_ci")?;
    write_rust_fixture(&target, false, true, true, false)?;
    Ok(RepairBenchCase {
        case_id: "missing_ci".to_string(),
        kind: "missing_ci".to_string(),
        target_path: target,
        expected_problem: Some("missing_ci".to_string()),
        fix_only: Some(FixOnly::Ci),
        apply: true,
        expected_files: vec![".github/workflows/rust-ci.yml".to_string()],
        validation_expected: true,
        notes: Vec::new(),
    })
}

fn build_missing_smoke_case(bench_id: &str, work_root: &Path) -> Result<RepairBenchCase, String> {
    let target = create_case_root(work_root, bench_id, "missing_smoke_test")?;
    write_rust_fixture(&target, true, false, true, false)?;
    Ok(RepairBenchCase {
        case_id: "missing_smoke_test".to_string(),
        kind: "missing_smoke_test".to_string(),
        target_path: target,
        expected_problem: Some("missing_smoke_test".to_string()),
        fix_only: Some(FixOnly::Tests),
        apply: true,
        expected_files: vec!["tests/eve_smoke.rs".to_string()],
        validation_expected: true,
        notes: Vec::new(),
    })
}

fn build_readme_case(bench_id: &str, work_root: &Path) -> Result<RepairBenchCase, String> {
    let target = create_case_root(work_root, bench_id, "readme_missing_validation")?;
    write_rust_fixture(&target, true, true, false, false)?;
    Ok(RepairBenchCase {
        case_id: "readme_missing_validation".to_string(),
        kind: "readme_missing_validation".to_string(),
        target_path: target,
        expected_problem: Some("missing_readme_validation".to_string()),
        fix_only: Some(FixOnly::Docs),
        apply: true,
        expected_files: vec!["README.md".to_string()],
        validation_expected: true,
        notes: Vec::new(),
    })
}

fn build_missing_module_case(bench_id: &str, work_root: &Path) -> Result<RepairBenchCase, String> {
    let target = create_case_root(work_root, bench_id, "simple_missing_module")?;
    write_missing_module_fixture(&target)?;
    Ok(RepairBenchCase {
        case_id: "simple_missing_module".to_string(),
        kind: "simple_missing_module".to_string(),
        target_path: target,
        expected_problem: Some("cargo_check_failure".to_string()),
        fix_only: Some(FixOnly::CargoCheck),
        apply: true,
        expected_files: vec!["src/missing_module.rs".to_string()],
        validation_expected: true,
        notes: Vec::new(),
    })
}

fn build_unknown_empty_case(bench_id: &str, work_root: &Path) -> Result<RepairBenchCase, String> {
    let target = create_case_root(work_root, bench_id, "unknown_empty_project")?;
    fs::create_dir_all(&target).map_err(|error| format!("create {}: {error}", target.display()))?;
    Ok(RepairBenchCase {
        case_id: "unknown_empty_project".to_string(),
        kind: "unknown_empty_project".to_string(),
        target_path: target,
        expected_problem: None,
        fix_only: None,
        apply: true,
        expected_files: Vec::new(),
        validation_expected: false,
        notes: Vec::new(),
    })
}

fn build_missing_gitignore_case(
    bench_id: &str,
    work_root: &Path,
) -> Result<RepairBenchCase, String> {
    let target = create_case_root(work_root, bench_id, "missing_gitignore_target")?;
    write_rust_fixture(&target, true, true, true, false)?;
    fs::write(target.join(".gitignore"), "*.log\n")
        .map_err(|error| format!("write {}: {error}", target.join(".gitignore").display()))?;
    Ok(RepairBenchCase {
        case_id: "missing_gitignore_target".to_string(),
        kind: "missing_gitignore_target".to_string(),
        target_path: target,
        expected_problem: Some("missing_gitignore_target".to_string()),
        fix_only: Some(FixOnly::Hygiene),
        apply: true,
        expected_files: vec![".gitignore".to_string()],
        validation_expected: true,
        notes: Vec::new(),
    })
}

fn build_missing_clippy_case(bench_id: &str, work_root: &Path) -> Result<RepairBenchCase, String> {
    let target = create_case_root(work_root, bench_id, "missing_clippy_ci")?;
    write_rust_fixture(&target, true, true, true, false)?;
    fs::write(
        target.join(".github/workflows/rust-ci.yml"),
        "name: Rust CI\non: [push]\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - run: cargo fmt --check\n      - run: cargo check --all-targets\n      - run: cargo test\n",
    )
    .map_err(|error| format!("write {}: {error}", target.join(".github/workflows/rust-ci.yml").display()))?;
    Ok(RepairBenchCase {
        case_id: "missing_clippy_ci".to_string(),
        kind: "missing_clippy_ci".to_string(),
        target_path: target,
        expected_problem: Some("missing_clippy_ci".to_string()),
        fix_only: Some(FixOnly::Ci),
        apply: true,
        expected_files: vec![".github/workflows/rust-ci.yml".to_string()],
        validation_expected: true,
        notes: Vec::new(),
    })
}

fn build_missing_readme_usage_case(
    bench_id: &str,
    work_root: &Path,
) -> Result<RepairBenchCase, String> {
    let target = create_case_root(work_root, bench_id, "missing_readme_usage_section")?;
    write_rust_fixture(&target, true, true, true, false)?;
    fs::write(
        target.join("README.md"),
        "# Fixture\n\n## Validation\n\n```bash\ncargo fmt --check\ncargo check\ncargo test\n```\n",
    )
    .map_err(|error| format!("write {}: {error}", target.join("README.md").display()))?;
    Ok(RepairBenchCase {
        case_id: "missing_readme_usage_section".to_string(),
        kind: "missing_readme_usage_section".to_string(),
        target_path: target,
        expected_problem: Some("missing_readme_usage_section".to_string()),
        fix_only: Some(FixOnly::Docs),
        apply: true,
        expected_files: vec!["README.md".to_string()],
        validation_expected: true,
        notes: Vec::new(),
    })
}

fn create_case_root(work_root: &Path, bench_id: &str, case_id: &str) -> Result<PathBuf, String> {
    let root = work_root.join(format!(
        "{bench_id}-{case_id}-{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).map_err(|error| format!("create {}: {error}", root.display()))?;
    Ok(root)
}

fn write_rust_fixture(
    root: &Path,
    ci: bool,
    smoke: bool,
    readme_validation: bool,
    missing_module: bool,
) -> Result<(), String> {
    fs::create_dir_all(root.join("src"))
        .map_err(|error| format!("create {}: {error}", root.join("src").display()))?;
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname=\"repair_bench_fixture\"\nversion=\"0.1.0\"\nedition=\"2021\"\n",
    )
    .map_err(|error| format!("write {}: {error}", root.join("Cargo.toml").display()))?;
    if missing_module {
        fs::write(
            root.join("src/lib.rs"),
            "mod missing_module;\n\npub fn marker() -> &'static str {\n    \"ok\"\n}\n",
        )
        .map_err(|error| format!("write {}: {error}", root.join("src/lib.rs").display()))?;
    } else {
        fs::write(
            root.join("src/lib.rs"),
            "pub fn marker() -> &'static str {\n    \"ok\"\n}\n",
        )
        .map_err(|error| format!("write {}: {error}", root.join("src/lib.rs").display()))?;
    }
    if ci {
        fs::create_dir_all(root.join(".github/workflows")).map_err(|error| {
            format!(
                "create {}: {error}",
                root.join(".github/workflows").display()
            )
        })?;
        fs::write(
            root.join(".github/workflows/rust-ci.yml"),
            "name: Rust CI\non: [push]\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - run: cargo fmt --check\n      - run: cargo check --all-targets\n      - run: cargo test\n",
        )
        .map_err(|error| format!("write {}: {error}", root.join(".github/workflows/rust-ci.yml").display()))?;
    }
    if smoke {
        fs::create_dir_all(root.join("tests"))
            .map_err(|error| format!("create {}: {error}", root.join("tests").display()))?;
        fs::write(
            root.join("tests/eve_smoke.rs"),
            "#[test]\nfn smoke() {\n    assert!(true);\n}\n",
        )
        .map_err(|error| {
            format!(
                "write {}: {error}",
                root.join("tests/eve_smoke.rs").display()
            )
        })?;
    }
    if readme_validation {
        fs::write(
            root.join("README.md"),
            "# Fixture\n\n## Validation\n\n- cargo fmt --check\n- cargo check\n- cargo test\n",
        )
        .map_err(|error| format!("write {}: {error}", root.join("README.md").display()))?;
    } else {
        fs::write(root.join("README.md"), "# Fixture\n")
            .map_err(|error| format!("write {}: {error}", root.join("README.md").display()))?;
    }
    Ok(())
}

fn write_missing_module_fixture(root: &Path) -> Result<(), String> {
    write_rust_fixture(root, true, true, true, true)
}

fn build_evidence_paths(request: &RepairBenchRequest) -> RepairBenchEvidencePaths {
    let output_dir = request.output_dir.join(&request.bench_id);
    RepairBenchEvidencePaths {
        request_json: output_dir.join("request.json"),
        report_json: output_dir.join("report.json"),
        report_md: output_dir.join("report.md"),
        cases_dir: output_dir.join("cases"),
        output_dir,
    }
}

fn summarize_metrics(results: &[RepairBenchCaseResult]) -> RepairBenchMetricSummary {
    let total_cases = results.len();
    let passed_cases = results
        .iter()
        .filter(|result| matches!(result.status, RepairBenchCaseStatus::Passed))
        .count();
    let failed_cases = results
        .iter()
        .filter(|result| matches!(result.status, RepairBenchCaseStatus::Failed))
        .count();
    let partial_cases = results
        .iter()
        .filter(|result| matches!(result.status, RepairBenchCaseStatus::Partial))
        .count();

    let actionable = results
        .iter()
        .filter(|result| result.expected_problem.is_some())
        .count();
    let detection_success = results
        .iter()
        .filter(|result| result.expected_problem.is_some())
        .filter(|result| result.detected_problem == result.expected_problem)
        .count();
    let repair_success = results
        .iter()
        .filter(|result| result.expected_problem.is_some())
        .filter(|result| !result.files_expected.is_empty())
        .filter(|result| {
            result.files_expected.iter().all(|file| {
                result
                    .files_observed
                    .iter()
                    .any(|observed| observed == file)
            })
        })
        .count();
    let validation_success = results
        .iter()
        .filter(|result| result.expected_problem.is_some())
        .filter(|result| result.validation_passed)
        .count();
    let evidence_success = results
        .iter()
        .filter(|result| {
            result
                .evidence_dir
                .as_ref()
                .map(|path| path.exists())
                .unwrap_or(false)
        })
        .count();

    RepairBenchMetricSummary {
        total_cases,
        actionable_cases: actionable,
        passed_cases,
        failed_cases,
        partial_cases,
        detection_success_rate: ratio(detection_success, actionable),
        repair_success_rate: ratio(repair_success, actionable),
        validation_success_rate: ratio(validation_success, actionable),
        evidence_success_rate: ratio(evidence_success, total_cases),
    }
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn render_report(report: &RepairBenchReport) -> String {
    let status = match report.status {
        RepairBenchStatus::Passed => "passed",
        RepairBenchStatus::Warn => "warn",
        RepairBenchStatus::Failed => "failed",
    };
    let mut output = String::new();
    output.push_str("EVE Repair Bench Report\n\n");
    output.push_str(&format!(
        "Suite: {}\nStatus: {}\nCases: {} total, {} passed, {} partial, {} failed\n\nResults:\n",
        report.suite,
        status,
        report.total_cases,
        report.passed_cases,
        report.partial_cases,
        report.failed_cases
    ));
    for result in &report.case_results {
        output.push_str(&format!(
            "  [{}] {:<24} detected={:<24} validation={}\n",
            case_status_label(&result.status),
            result.kind,
            result.detected_problem.as_deref().unwrap_or("none"),
            if result.validation_passed {
                "passed"
            } else {
                "not_run"
            }
        ));
    }
    output.push_str(&format!("\nOutput:\n  {}\n", report.output_dir.display()));
    output.push_str(&format!(
        "\nMetrics:\n  total_cases={}\n  actionable_cases={}\n  passed_cases={}\n  failed_cases={}\n  partial_cases={}\n  detection_success_rate={:.2}\n  repair_success_rate={:.2}\n  validation_success_rate={:.2}\n  evidence_success_rate={:.2}\n",
        report.metrics.total_cases,
        report.metrics.actionable_cases,
        report.metrics.passed_cases,
        report.metrics.failed_cases,
        report.metrics.partial_cases,
        report.metrics.detection_success_rate,
        report.metrics.repair_success_rate,
        report.metrics.validation_success_rate,
        report.metrics.evidence_success_rate,
    ));
    output
}

fn write_report_markdown(path: &Path, report: &RepairBenchReport) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    fs::write(path, render_report(report))
        .map_err(|error| format!("write {}: {error}", path.display()))
}

fn write_gate_report_markdown(path: &Path, report: &RepairBenchGateReport) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    fs::write(path, render_gate_report(report))
        .map_err(|error| format!("write {}: {error}", path.display()))
}

fn fix_status_label(report: &FixReport) -> String {
    match report.status {
        FixStatus::Detected => "detected",
        FixStatus::ProposalCreated => "proposal_created",
        FixStatus::DryRunPassed => "dry_run_passed",
        FixStatus::DryRunFailed => "dry_run_failed",
        FixStatus::Applied => "applied",
        FixStatus::ValidationPassed => "validation_passed",
        FixStatus::ValidationFailed => "validation_failed",
        FixStatus::NoActionableProblem => "no_actionable_problem",
        FixStatus::Blocked => "blocked",
    }
    .to_string()
}

fn case_status_label(status: &RepairBenchCaseStatus) -> &'static str {
    match status {
        RepairBenchCaseStatus::Passed => "passed",
        RepairBenchCaseStatus::Failed => "failed",
        RepairBenchCaseStatus::Partial => "partial",
        RepairBenchCaseStatus::Blocked => "blocked",
    }
}

fn observed_files(target_path: &Path, expected_files: &[String]) -> Vec<String> {
    expected_files
        .iter()
        .filter(|file| target_path.join(file).exists())
        .cloned()
        .collect()
}

fn append_history_entry(output_dir: &Path, report: &RepairBenchReport) -> Result<(), String> {
    let paths = history_paths(output_dir);
    if let Some(parent) = paths.history_jsonl.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    let entry = history_entry_from_report(report);
    let line = serde_json::to_string(&entry).map_err(|error| error.to_string())?;
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.history_jsonl)
        .and_then(|mut file| {
            use std::io::Write;
            writeln!(file, "{line}")
        })
        .map_err(|error| format!("append {}: {error}", paths.history_jsonl.display()))?;
    save_json_pretty(&paths.latest_json, &entry)?;
    Ok(())
}

fn history_entry_from_report(report: &RepairBenchReport) -> RepairBenchHistoryEntry {
    RepairBenchHistoryEntry {
        bench_id: report.bench_id.clone(),
        suite: report.suite.clone(),
        status: report.status.clone(),
        total_cases: report.total_cases,
        passed_cases: report.passed_cases,
        partial_cases: report.partial_cases,
        failed_cases: report.failed_cases,
        detection_success_rate: report.metrics.detection_success_rate,
        repair_success_rate: report.metrics.repair_success_rate,
        validation_success_rate: report.metrics.validation_success_rate,
        evidence_success_rate: report.metrics.evidence_success_rate,
        case_statuses: report
            .case_results
            .iter()
            .map(|case| RepairBenchCaseHistoryStatus {
                case_id: case.case_id.clone(),
                status: case.status.clone(),
            })
            .collect(),
        generated_at: now_unix() as i64,
    }
}

fn history_paths(output_dir: &Path) -> RepairBenchHistoryPaths {
    RepairBenchHistoryPaths {
        history_jsonl: output_dir.join("history.jsonl"),
        latest_json: output_dir.join("latest.json"),
    }
}

fn load_history_entries(
    path: &Path,
    limit: Option<usize>,
) -> Result<Vec<RepairBenchHistoryEntry>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents =
        fs::read_to_string(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let mut entries = Vec::new();
    for line in contents.lines().filter(|line| !line.trim().is_empty()) {
        let entry: RepairBenchHistoryEntry =
            serde_json::from_str(line).map_err(|error| format!("parse history line: {error}"))?;
        entries.push(entry);
    }
    if let Some(limit) = limit {
        if entries.len() > limit {
            entries = entries[entries.len() - limit..].to_vec();
        }
    }
    Ok(entries)
}

fn render_history_report(report: &RepairBenchHistoryReport, limit: Option<usize>) -> String {
    let entries = if let Some(limit) = limit {
        if report.entries.len() > limit {
            report.entries[report.entries.len() - limit..].to_vec()
        } else {
            report.entries.clone()
        }
    } else {
        report.entries.clone()
    };
    let mut output = String::from("EVE Repair Bench History\n\n");
    output.push_str(&format!("Runs: {}\n", report.runs));
    if let Some(latest) = entries.last() {
        output.push_str("\nLatest:\n");
        output.push_str(&format!("  bench_id: {}\n", latest.bench_id));
        output.push_str(&format!("  suite: {}\n", latest.suite));
        output.push_str(&format!(
            "  status: {}\n",
            repair_bench_status_label(&latest.status)
        ));
        output.push_str(&format!(
            "  cases: {} total, {} passed, {} partial, {} failed\n",
            latest.total_cases, latest.passed_cases, latest.partial_cases, latest.failed_cases
        ));
        output.push_str(&format!(
            "  repair_success_rate: {:.2}\n",
            latest.repair_success_rate
        ));
    } else {
        output.push_str("Status: empty\n");
    }
    output
}

fn render_gate_report(report: &RepairBenchGateReport) -> String {
    let mut output = String::from("EVE Repair Bench Gate\n\n");
    output.push_str(&format!("Suite: {}\n", report.suite));
    output.push_str(&format!(
        "Status: {}\n\n",
        repair_bench_gate_status_label(&report.status)
    ));
    output.push_str(&format!(
        "Baseline:\n  {} total, {} passed, {} partial, {} failed\n\n",
        report.baseline.total_cases,
        report.baseline.passed_cases,
        report.baseline.partial_cases,
        report.baseline.failed_cases
    ));
    output.push_str(&format!(
        "Current:\n  {} total, {} passed, {} partial, {} failed\n\n",
        report.current.total_cases,
        report.current.passed_cases,
        report.current.partial_cases,
        report.current.failed_cases
    ));
    output.push_str("Regression:\n");
    if report.regressions.is_empty() {
        output.push_str("  none\n");
    } else {
        for regression in &report.regressions {
            output.push_str(&format!(
                "  {}: baseline={} current={}\n",
                regression.field, regression.baseline, regression.current
            ));
        }
    }
    output.push_str(&format!(
        "\nExit:\n  {}\n",
        if matches!(
            report.status,
            RepairBenchGateStatus::Failed | RepairBenchGateStatus::Blocked
        ) {
            "non-zero"
        } else {
            "zero"
        }
    ));
    output
}

pub fn benchmark_from_report(report: &RepairBenchReport) -> RepairBenchBaseline {
    RepairBenchBaseline {
        suite: report.suite.clone(),
        total_cases: report.total_cases,
        actionable_cases: report.metrics.actionable_cases,
        passed_cases: report.passed_cases,
        partial_cases: report.partial_cases,
        failed_cases: report.failed_cases,
        detection_success_rate: report.metrics.detection_success_rate,
        repair_success_rate: report.metrics.repair_success_rate,
        validation_success_rate: report.metrics.validation_success_rate,
        evidence_success_rate: report.metrics.evidence_success_rate,
    }
}

fn load_repair_bench_baseline(
    request: &RepairBenchGateRequest,
) -> Result<RepairBenchBaseline, String> {
    if let Some(path) = &request.baseline_file {
        let contents = fs::read_to_string(path)
            .map_err(|error| format!("read {}: {error}", path.display()))?;
        return serde_json::from_str(&contents)
            .map_err(|error| format!("parse baseline {}: {error}", path.display()));
    }
    match request.baseline.as_str() {
        "latest" => {
            let paths = history_paths(&request.output_dir);
            if paths.latest_json.exists() {
                let contents = fs::read_to_string(&paths.latest_json)
                    .map_err(|error| format!("read {}: {error}", paths.latest_json.display()))?;
                let latest: RepairBenchHistoryEntry = serde_json::from_str(&contents)
                    .map_err(|error| format!("parse {}: {error}", paths.latest_json.display()))?;
                let latest_suite = latest.suite.clone();
                Ok(RepairBenchBaseline {
                    suite: latest_suite.clone(),
                    total_cases: latest.total_cases,
                    actionable_cases: actionable_cases_for_suite(&latest_suite),
                    passed_cases: latest.passed_cases,
                    partial_cases: latest.partial_cases,
                    failed_cases: latest.failed_cases,
                    detection_success_rate: latest.detection_success_rate,
                    repair_success_rate: latest.repair_success_rate,
                    validation_success_rate: latest.validation_success_rate,
                    evidence_success_rate: latest.evidence_success_rate,
                })
            } else {
                Ok(default_phase21_baseline())
            }
        }
        "" => Ok(default_phase21_baseline()),
        _ => Ok(default_phase21_baseline()),
    }
}

fn default_phase21_baseline() -> RepairBenchBaseline {
    RepairBenchBaseline {
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
    }
}

fn actionable_cases_for_suite(suite: &str) -> usize {
    match suite {
        "phase24x" => 7,
        _ => 4,
    }
}

pub fn detect_repair_bench_regressions(
    baseline: &RepairBenchBaseline,
    current: &RepairBenchBaseline,
    current_report: &RepairBenchReport,
) -> Vec<RepairBenchRegression> {
    let mut regressions = Vec::new();
    if current.failed_cases > baseline.failed_cases {
        regressions.push(RepairBenchRegression {
            field: "failed_cases increased".to_string(),
            baseline: baseline.failed_cases.to_string(),
            current: current.failed_cases.to_string(),
        });
    }
    if current.passed_cases < baseline.passed_cases {
        regressions.push(RepairBenchRegression {
            field: "passed_cases decreased".to_string(),
            baseline: baseline.passed_cases.to_string(),
            current: current.passed_cases.to_string(),
        });
    }
    if current.detection_success_rate < baseline.detection_success_rate {
        regressions.push(RepairBenchRegression {
            field: "detection_success_rate decreased".to_string(),
            baseline: format!("{:.2}", baseline.detection_success_rate),
            current: format!("{:.2}", current.detection_success_rate),
        });
    }
    if current.repair_success_rate < baseline.repair_success_rate {
        regressions.push(RepairBenchRegression {
            field: "repair_success_rate decreased".to_string(),
            baseline: format!("{:.2}", baseline.repair_success_rate),
            current: format!("{:.2}", current.repair_success_rate),
        });
    }
    if current.validation_success_rate < baseline.validation_success_rate {
        regressions.push(RepairBenchRegression {
            field: "validation_success_rate decreased".to_string(),
            baseline: format!("{:.2}", baseline.validation_success_rate),
            current: format!("{:.2}", current.validation_success_rate),
        });
    }
    if current.evidence_success_rate < baseline.evidence_success_rate {
        regressions.push(RepairBenchRegression {
            field: "evidence_success_rate decreased".to_string(),
            baseline: format!("{:.2}", baseline.evidence_success_rate),
            current: format!("{:.2}", current.evidence_success_rate),
        });
    }
    if report_has_actionable_failure(current_report) {
        regressions.push(RepairBenchRegression {
            field: "actionable_case_failed".to_string(),
            baseline: "none".to_string(),
            current: "failed".to_string(),
        });
    }
    regressions
}

fn report_has_actionable_failure(report: &RepairBenchReport) -> bool {
    report
        .case_results
        .iter()
        .any(|case| match &case.expected_problem {
            Some(_) => matches!(case.status, RepairBenchCaseStatus::Failed),
            None => false,
        })
}

fn gate_status_for(
    baseline: &RepairBenchBaseline,
    current_report: &RepairBenchReport,
    regressions: &[RepairBenchRegression],
) -> RepairBenchGateStatus {
    if current_report.total_cases == 0 {
        return RepairBenchGateStatus::Blocked;
    }
    if regressions.is_empty() {
        RepairBenchGateStatus::Passed
    } else if baseline.suite != current_report.suite {
        RepairBenchGateStatus::Warn
    } else {
        RepairBenchGateStatus::Failed
    }
}

fn repair_bench_status_label(status: &RepairBenchStatus) -> &'static str {
    match status {
        RepairBenchStatus::Passed => "passed",
        RepairBenchStatus::Warn => "warn",
        RepairBenchStatus::Failed => "failed",
    }
}

fn repair_bench_gate_status_label(status: &RepairBenchGateStatus) -> &'static str {
    match status {
        RepairBenchGateStatus::Passed => "passed",
        RepairBenchGateStatus::Warn => "warn",
        RepairBenchGateStatus::Failed => "failed",
        RepairBenchGateStatus::Blocked => "blocked",
    }
}

#[derive(Debug, Clone)]
struct RepairBenchHistoryPaths {
    history_jsonl: PathBuf,
    latest_json: PathBuf,
}
