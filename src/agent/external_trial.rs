use crate::agent::doctor::run_doctor;
use crate::agent::fix::run_fix;
use crate::agent::storage::{id, save_json_pretty};
use crate::contracts::{
    parse_fix_only_from_command, DoctorRequest, ExternalTrialRepoResult, ExternalTrialReport,
    ExternalTrialRequest, FixRequest, FixRiskCap,
};
use std::fs;
use std::path::Path;

pub fn run_external_trial(request: ExternalTrialRequest) -> Result<ExternalTrialReport, String> {
    let mut repo_results = Vec::with_capacity(request.repo_paths.len());
    let mut processed = 0usize;
    let mut skipped = 0usize;
    let mut warnings = Vec::new();
    let mut blockers = Vec::new();

    for repo_path in &request.repo_paths {
        let repo_result = run_external_trial_repo(repo_path, &request.output_dir)?;
        if matches!(repo_result.status.as_str(), "skipped") {
            skipped += 1;
            blockers.extend(repo_result.notes.iter().filter_map(|note| {
                note.starts_with("blocker:")
                    .then(|| note.trim_start_matches("blocker:").to_string())
            }));
        } else {
            processed += 1;
        }
        if !repo_result.notes.is_empty() {
            warnings.extend(
                repo_result
                    .notes
                    .iter()
                    .filter(|note| note.starts_with("warning:"))
                    .cloned(),
            );
        }
        repo_results.push(repo_result);
    }

    let report = ExternalTrialReport {
        trial_id: request.trial_id.clone(),
        repos_total: request.repo_paths.len(),
        repos_processed: processed,
        repos_skipped: skipped,
        repo_results,
        warnings,
        blockers,
        output_dir: request.output_dir.clone(),
    };

    fs::create_dir_all(&request.output_dir)
        .map_err(|error| format!("create {}: {error}", request.output_dir.display()))?;
    save_json_pretty(&request.output_dir.join("report.json"), &report)?;
    fs::write(
        &request.output_dir.join("report.md"),
        render_external_trial_report(&report),
    )
    .map_err(|error| {
        format!(
            "write {}: {error}",
            request.output_dir.join("report.md").display()
        )
    })?;
    Ok(report)
}

pub fn print_external_trial(request: ExternalTrialRequest) -> Result<String, String> {
    let report = run_external_trial(request.clone())?;
    if request.json {
        serde_json::to_string_pretty(&report).map_err(|error| error.to_string())
    } else {
        Ok(render_external_trial_report(&report))
    }
}

fn run_external_trial_repo(
    repo_path: &Path,
    output_dir: &Path,
) -> Result<ExternalTrialRepoResult, String> {
    let repo_name = repo_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| "repo".to_string());
    let exists = repo_path.exists();
    let is_directory = repo_path.is_dir();
    let evidence_dir = output_dir.join(&repo_name);

    if !exists {
        return Ok(ExternalTrialRepoResult {
            repo_path: repo_path.to_path_buf(),
            repo_name,
            exists,
            is_directory,
            doctor_status: "skipped".to_string(),
            doctor_health_score: 0,
            doctor_findings: Vec::new(),
            suggested_fix_commands: Vec::new(),
            dry_run_fix_reports: Vec::new(),
            source_mutation: false,
            evidence_dir,
            status: "skipped".to_string(),
            notes: vec!["blocker:target_path_does_not_exist".to_string()],
        });
    }
    if !is_directory {
        return Ok(ExternalTrialRepoResult {
            repo_path: repo_path.to_path_buf(),
            repo_name,
            exists,
            is_directory,
            doctor_status: "skipped".to_string(),
            doctor_health_score: 0,
            doctor_findings: Vec::new(),
            suggested_fix_commands: Vec::new(),
            dry_run_fix_reports: Vec::new(),
            source_mutation: false,
            evidence_dir,
            status: "skipped".to_string(),
            notes: vec!["blocker:target_path_not_directory".to_string()],
        });
    }

    fs::create_dir_all(&evidence_dir)
        .map_err(|error| format!("create {}: {error}", evidence_dir.display()))?;
    let doctor_request = DoctorRequest {
        doctor_id: id("external-trial-doctor"),
        target_path: repo_path.to_path_buf(),
        validate: false,
        json: false,
        no_llm: true,
        evidence_dir: evidence_dir.join("doctor"),
    };
    let doctor_report = run_doctor(doctor_request)?;
    let suggested_fix_commands = doctor_report
        .suggestions
        .iter()
        .map(|suggestion| suggestion.command.clone())
        .collect::<Vec<_>>();

    let mut dry_run_fix_reports = Vec::new();
    for (index, command) in suggested_fix_commands.iter().enumerate() {
        if let Some(only) = parse_fix_only_from_command(command) {
            let fix_report = run_fix(FixRequest {
                fix_id: format!("{}-{index}", id("external-trial-fix")),
                target_path: repo_path.to_path_buf(),
                dry_run: true,
                apply: false,
                only: Some(only),
                max_files: 3,
                risk_cap: FixRiskCap::Low,
                no_llm: true,
                provider: Some("rule_based".to_string()),
                evidence_dir: evidence_dir.join(format!("fix-dry-run-{index}")),
            })?;
            dry_run_fix_reports.push(fix_report);
        }
    }

    let source_mutation = dry_run_fix_reports
        .iter()
        .any(|report| report.source_mutation);
    Ok(ExternalTrialRepoResult {
        repo_path: repo_path.to_path_buf(),
        repo_name,
        exists,
        is_directory,
        doctor_status: format!("{:?}", doctor_report.status).to_lowercase(),
        doctor_health_score: doctor_report.health_score,
        doctor_findings: doctor_report.findings,
        suggested_fix_commands,
        dry_run_fix_reports,
        source_mutation,
        evidence_dir,
        status: "processed".to_string(),
        notes: Vec::new(),
    })
}

fn render_external_trial_report(report: &ExternalTrialReport) -> String {
    let mut output = String::from("EVE External Trial Report\n\n");
    output.push_str(&format!(
        "Repos: {} total, {} processed, {} skipped\n\nResults:\n",
        report.repos_total, report.repos_processed, report.repos_skipped
    ));
    for repo in &report.repo_results {
        let status_line = if repo.status == "skipped" {
            format!(
                "  [skipped]   {:<14} reason={}",
                repo.repo_name,
                repo.notes
                    .first()
                    .map(|note| note.trim_start_matches("blocker:"))
                    .unwrap_or("unknown")
            )
        } else {
            format!(
                "  [processed] {:<14} doctor={} suggested_fixes={} source_mutation={}",
                repo.repo_name,
                repo.doctor_status,
                repo.suggested_fix_commands.len(),
                repo.source_mutation
            )
        };
        output.push_str(&status_line);
        output.push('\n');
    }
    output.push_str(&format!("\nOutput:\n  {}\n", report.output_dir.display()));
    output
}
