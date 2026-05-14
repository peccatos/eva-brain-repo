use crate::agent::storage::save_json_pretty;
use crate::agent::validate::run_cargo;
use crate::contracts::{
    DoctorEvidencePaths, DoctorFinding, DoctorFindingLevel, DoctorProjectType, DoctorReport,
    DoctorRequest, DoctorStatus, DoctorSuggestion, DoctorValidationSummary,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

pub fn run_doctor(request: DoctorRequest) -> Result<DoctorReport, String> {
    if let Some(report) = blocked_target_report(&request) {
        return Ok(report);
    }

    let target_root = request
        .target_path
        .canonicalize()
        .unwrap_or(request.target_path.clone());
    let project_type = if target_root.join("Cargo.toml").exists() {
        DoctorProjectType::Rust
    } else {
        DoctorProjectType::Unknown
    };
    let workspace_changes = git_status_short(&target_root);
    let workspace_dirty = !workspace_changes.is_empty();
    let mut findings = build_findings(&target_root, &project_type, workspace_dirty);
    let suggestions = build_suggestions(&findings, &target_root);
    let mut health_score = score_findings(&findings);

    let validation = if request.validate && matches!(project_type, DoctorProjectType::Rust) {
        Some(run_validation(&target_root, &workspace_changes))
    } else {
        None
    };

    let mut warnings = Vec::new();
    let mut blockers = Vec::new();
    if workspace_dirty {
        warnings.push("workspace_dirty".to_string());
    }
    if let Some(validation) = &validation {
        warnings.extend(validation_warnings(validation));
        if validation.commands.iter().any(|command| !command.success) {
            findings.push(DoctorFinding {
                level: DoctorFindingLevel::Critical,
                code: "validation_failed".to_string(),
                message: "validation commands failed".to_string(),
            });
            blockers.push("validation_failed".to_string());
            health_score = health_score.saturating_sub(40);
        }
    }

    if findings
        .iter()
        .any(|finding| matches!(finding.level, DoctorFindingLevel::Critical))
    {
        health_score = health_score.min(60);
    }

    let status = if findings
        .iter()
        .any(|finding| matches!(finding.level, DoctorFindingLevel::Critical))
    {
        DoctorStatus::Critical
    } else if health_score >= 85 {
        DoctorStatus::Passed
    } else if health_score >= 60 {
        DoctorStatus::Warn
    } else {
        DoctorStatus::Critical
    };

    let evidence = build_evidence_paths(&request, &target_root);
    save_json_pretty(&evidence.request_json, &request)?;

    let report = DoctorReport {
        doctor_id: request.doctor_id.clone(),
        target_path: target_root,
        project_type,
        status,
        health_score,
        workspace_dirty,
        source_mutation: false,
        evidence_written: true,
        findings,
        suggestions,
        validation,
        evidence_dir: Some(evidence.evidence_dir.clone()),
        warnings,
        blockers,
    };

    write_report_files(&evidence, &report)?;
    Ok(report)
}

pub fn print_doctor(request: DoctorRequest) -> Result<String, String> {
    let report = run_doctor(request.clone())?;
    if request.json {
        serde_json::to_string_pretty(&report).map_err(|error| error.to_string())
    } else {
        Ok(render_doctor_report(&report))
    }
}

fn blocked_target_report(request: &DoctorRequest) -> Option<DoctorReport> {
    if !request.target_path.exists() {
        return Some(blocked_report(
            request,
            "target_path_does_not_exist",
            request.target_path.clone(),
        ));
    }
    if !request.target_path.is_dir() {
        return Some(blocked_report(
            request,
            "target_path_not_directory",
            request.target_path.clone(),
        ));
    }
    None
}

fn blocked_report(request: &DoctorRequest, blocker: &str, target_path: PathBuf) -> DoctorReport {
    DoctorReport {
        doctor_id: request.doctor_id.clone(),
        target_path,
        project_type: DoctorProjectType::Unknown,
        status: DoctorStatus::Blocked,
        health_score: 0,
        workspace_dirty: false,
        source_mutation: false,
        evidence_written: false,
        findings: Vec::new(),
        suggestions: Vec::new(),
        validation: None,
        evidence_dir: None,
        warnings: Vec::new(),
        blockers: vec![blocker.to_string()],
    }
}

fn build_findings(
    target_root: &Path,
    project_type: &DoctorProjectType,
    workspace_dirty: bool,
) -> Vec<DoctorFinding> {
    let mut findings = Vec::new();
    let cargo_toml = target_root.join("Cargo.toml");
    if cargo_toml.exists() {
        findings.push(DoctorFinding {
            level: DoctorFindingLevel::Ok,
            code: "cargo_toml_found".to_string(),
            message: "Cargo.toml found".to_string(),
        });
    } else {
        findings.push(DoctorFinding {
            level: DoctorFindingLevel::Critical,
            code: "cargo_toml_missing".to_string(),
            message: "no Cargo.toml found".to_string(),
        });
        findings.push(DoctorFinding {
            level: DoctorFindingLevel::Info,
            code: "readme_missing".to_string(),
            message: "README missing".to_string(),
        });
        return findings;
    }

    if matches!(project_type, DoctorProjectType::Rust) {
        if target_root.join(".github/workflows/rust-ci.yml").exists() {
            findings.push(DoctorFinding {
                level: DoctorFindingLevel::Ok,
                code: "rust_ci_found".to_string(),
                message: "Rust CI workflow found".to_string(),
            });
        } else {
            findings.push(DoctorFinding {
                level: DoctorFindingLevel::Warn,
                code: "rust_ci_missing".to_string(),
                message: "Rust CI workflow missing".to_string(),
            });
        }

        if target_root.join("tests/eve_smoke.rs").exists() {
            findings.push(DoctorFinding {
                level: DoctorFindingLevel::Ok,
                code: "smoke_test_found".to_string(),
                message: "smoke test found".to_string(),
            });
        } else {
            findings.push(DoctorFinding {
                level: DoctorFindingLevel::Warn,
                code: "smoke_test_missing".to_string(),
                message: "smoke test missing".to_string(),
            });
        }

        let readme = target_root.join("README.md");
        if readme.exists() {
            let contents = fs::read_to_string(&readme).unwrap_or_default();
            if contents.contains("cargo fmt --check")
                && contents.contains("cargo check")
                && contents.contains("cargo test")
            {
                findings.push(DoctorFinding {
                    level: DoctorFindingLevel::Ok,
                    code: "readme_validation_found".to_string(),
                    message: "README validation section found".to_string(),
                });
            } else {
                findings.push(DoctorFinding {
                    level: DoctorFindingLevel::Warn,
                    code: "readme_validation_missing".to_string(),
                    message: "README validation section missing".to_string(),
                });
            }
        } else {
            findings.push(DoctorFinding {
                level: DoctorFindingLevel::Info,
                code: "readme_missing".to_string(),
                message: "README missing".to_string(),
            });
        }
    }

    if workspace_dirty {
        findings.push(DoctorFinding {
            level: DoctorFindingLevel::Warn,
            code: "workspace_dirty".to_string(),
            message: "workspace dirty".to_string(),
        });
    } else {
        findings.push(DoctorFinding {
            level: DoctorFindingLevel::Ok,
            code: "workspace_clean".to_string(),
            message: "workspace clean".to_string(),
        });
    }

    findings
}

fn build_suggestions(findings: &[DoctorFinding], target_root: &Path) -> Vec<DoctorSuggestion> {
    let target = display_target(target_root);
    let mut suggestions = Vec::new();
    for finding in findings {
        match finding.code.as_str() {
            "rust_ci_missing" => suggestions.push(DoctorSuggestion {
                command: format!("cargo run -- fix {target} --only ci"),
                reason: "add the missing Rust CI workflow".to_string(),
            }),
            "smoke_test_missing" => suggestions.push(DoctorSuggestion {
                command: format!("cargo run -- fix {target} --only tests"),
                reason: "add a smoke test".to_string(),
            }),
            "readme_validation_missing" => suggestions.push(DoctorSuggestion {
                command: format!("cargo run -- fix {target} --only docs"),
                reason: "add a README validation section".to_string(),
            }),
            _ => {}
        }
    }
    suggestions
}

fn run_validation(
    target_root: &Path,
    workspace_status_before: &[String],
) -> DoctorValidationSummary {
    let cargo_lock_before = target_root.join("Cargo.lock").exists();
    let commands = vec![
        run_cargo(target_root.to_str().unwrap_or("."), &["fmt", "--check"]),
        run_cargo(
            target_root.to_str().unwrap_or("."),
            &["check", "--all-targets"],
        ),
        run_cargo(target_root.to_str().unwrap_or("."), &["test"]),
    ];
    let git_status_after = git_status_short(target_root);
    let cargo_lock_after = target_root.join("Cargo.lock").exists();
    let mut validation_side_effects = validation_side_effects(
        workspace_status_before,
        &git_status_after,
        cargo_lock_before,
        cargo_lock_after,
    );
    validation_side_effects.sort();
    validation_side_effects.dedup();
    let status = if commands.iter().all(|command| command.success) {
        "passed"
    } else if commands.iter().any(|command| command.success) {
        "warn"
    } else {
        "failed"
    }
    .to_string();
    DoctorValidationSummary {
        status,
        commands,
        git_status_before: workspace_status_before.to_vec(),
        git_status_after,
        validation_side_effects,
    }
}

fn validation_side_effects(
    before: &[String],
    after: &[String],
    cargo_lock_before: bool,
    cargo_lock_after: bool,
) -> Vec<String> {
    let before_paths: BTreeSet<String> = before.iter().map(|line| status_path(line)).collect();
    let mut side_effects = Vec::new();
    for path in after.iter().map(|line| status_path(line)) {
        if !before_paths.contains(&path) {
            side_effects.push(path);
        }
    }
    if !cargo_lock_before
        && cargo_lock_after
        && !side_effects.iter().any(|path| path == "Cargo.lock")
    {
        side_effects.push("Cargo.lock".to_string());
    }
    side_effects
}

fn score_findings(findings: &[DoctorFinding]) -> u8 {
    let mut score = 100i32;
    for finding in findings {
        score -= match finding.level {
            DoctorFindingLevel::Ok => 0,
            DoctorFindingLevel::Info => 2,
            DoctorFindingLevel::Warn => 10,
            DoctorFindingLevel::Critical => 40,
        };
    }
    score.clamp(0, 100) as u8
}

fn render_doctor_report(report: &DoctorReport) -> String {
    let status = match report.status {
        DoctorStatus::Passed => "passed",
        DoctorStatus::Warn => "warn",
        DoctorStatus::Critical => "critical",
        DoctorStatus::Blocked => "blocked",
    };
    let project_type = match report.project_type {
        DoctorProjectType::Rust => "rust",
        DoctorProjectType::Unknown => "unknown",
    };
    let mut output = String::new();
    output.push_str("EVE Doctor Report\n\n");
    output.push_str(&format!(
        "Target: {}\nProject type: {}\nStatus: {}\nHealth: {}/100\nWorkspace dirty: {}\nSource mutation: {}\nEvidence written: {}\n",
        report.target_path.display(),
        project_type,
        status,
        report.health_score,
        report.workspace_dirty,
        report.source_mutation,
        report.evidence_written
    ));
    if !report.findings.is_empty() {
        output.push_str("\nFindings:\n");
        for finding in &report.findings {
            output.push_str(&format!(
                "  [{}] {}\n",
                finding_label(&finding.level),
                finding.message
            ));
        }
    }
    if !report.suggestions.is_empty() {
        output.push_str("\nSuggested fixes:\n");
        for suggestion in &report.suggestions {
            output.push_str(&format!("  {}\n", suggestion.command));
        }
    }
    if !report.warnings.is_empty() {
        output.push_str("\nWarnings:\n");
        for warning in &report.warnings {
            output.push_str(&format!("  {}\n", warning));
        }
    }
    if !report.blockers.is_empty() {
        output.push_str("\nBlockers:\n");
        for blocker in &report.blockers {
            output.push_str(&format!("  {}\n", blocker));
        }
    }
    output.push_str("\nEvidence:\n");
    if report.evidence_written {
        if let Some(path) = &report.evidence_dir {
            output.push_str(&format!("  {}\n", path.display()));
        } else {
            output.push_str("  none\n");
        }
    } else {
        output.push_str("  none\n");
    }
    if let Some(validation) = &report.validation {
        output.push_str(&format!("\nValidation:\n  {}\n", validation.status));
        if !validation.validation_side_effects.is_empty() {
            output.push_str("Validation side effects:\n");
            for item in &validation.validation_side_effects {
                output.push_str(&format!("  {}\n", item));
            }
        }
    }
    output
}

fn build_evidence_paths(request: &DoctorRequest, target_root: &Path) -> DoctorEvidencePaths {
    let evidence_root = if request.evidence_dir.is_absolute() {
        request.evidence_dir.clone()
    } else {
        target_root.join(&request.evidence_dir)
    };
    let evidence_dir = evidence_root.join(&request.doctor_id);
    DoctorEvidencePaths {
        request_json: evidence_dir.join("request.json"),
        report_json: evidence_dir.join("report.json"),
        report_md: evidence_dir.join("report.md"),
        validation_json: request
            .validate
            .then(|| evidence_dir.join("validation.json")),
        evidence_dir,
    }
}

fn write_report_files(evidence: &DoctorEvidencePaths, report: &DoctorReport) -> Result<(), String> {
    save_json_pretty(&evidence.report_json, report)?;
    if let (Some(validation_path), Some(validation)) =
        (&evidence.validation_json, &report.validation)
    {
        save_json_pretty(validation_path, validation)?;
    }
    if let Some(parent) = evidence.report_md.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    fs::write(&evidence.report_md, render_doctor_report(report))
        .map_err(|error| format!("write {}: {error}", evidence.report_md.display()))
}

fn finding_label(level: &DoctorFindingLevel) -> &'static str {
    match level {
        DoctorFindingLevel::Ok => "ok",
        DoctorFindingLevel::Info => "info",
        DoctorFindingLevel::Warn => "warn",
        DoctorFindingLevel::Critical => "critical",
    }
}

fn display_target(target_root: &Path) -> String {
    target_root.display().to_string()
}

fn status_path(line: &str) -> String {
    line.get(3..)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(line)
        .to_string()
}

fn git_status_short(target_root: &Path) -> Vec<String> {
    let output = std::process::Command::new("git")
        .args(["status", "--short"])
        .current_dir(target_root)
        .output();
    match output {
        Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn validation_warnings(summary: &DoctorValidationSummary) -> Vec<String> {
    if summary.commands.iter().any(|command| !command.success) {
        vec!["validation_failed".to_string()]
    } else {
        Vec::new()
    }
}
