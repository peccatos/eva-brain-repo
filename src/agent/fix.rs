use crate::agent::propose::{proposal_from_llm_response, validate_patch_proposal};
use crate::agent::safe_paths::validate_patch_path;
use crate::agent::storage::{id, now_unix, save_json_pretty};
use crate::agent::validate::run_cargo;
use crate::contracts::{
    AgentValidationStatus, FixDetectedProblem, FixEvidencePaths, FixMode, FixOnly, FixProblemKind,
    FixReport, FixRequest, FixRiskCap, FixStatus, LlmPurpose, LlmRequest, PatchOp,
    PatchOperationKind, PatchProposal, ProposalStatus, ValidationRun,
};
use crate::llm::prompts::AGENT_SYSTEM_PROMPT;
use crate::llm::schemas::PATCH_PROPOSAL_SCHEMA;
use crate::llm::{select_llm_provider_from_env, selected_llm_provider_name_from_env, LlmProvider};
use std::fs;
use std::path::{Path, PathBuf};

pub fn run_fix(request: FixRequest) -> Result<FixReport, String> {
    if let Some(report) = blocked_target_report(&request) {
        return Ok(report);
    }
    let target_root = request
        .target_path
        .canonicalize()
        .unwrap_or(request.target_path.clone());
    let workspace_changes = git_status_short(&target_root);
    let workspace_dirty = !workspace_changes.is_empty();
    let project_type = if target_root.join("Cargo.toml").exists() {
        "rust".to_string()
    } else {
        "unknown".to_string()
    };
    let evidence = build_evidence_paths(&request, &target_root);
    save_json_pretty(&evidence.request_json, &request)?;

    let Some(detected) = detect_problem(
        &target_root,
        &project_type,
        workspace_dirty,
        workspace_changes.clone(),
        request.only.clone(),
    )?
    else {
        let report = FixReport {
            fix_id: request.fix_id.clone(),
            target_path: target_root,
            mode: mode_for(&request),
            project_type,
            workspace_dirty,
            detected_problem: None,
            risk: "low".to_string(),
            files_planned: Vec::new(),
            files_changed_by_patch: Vec::new(),
            files_changed_after_validation: Vec::new(),
            validation_side_effects: Vec::new(),
            validation_commands: Vec::new(),
            status: FixStatus::NoActionableProblem,
            evidence_dir: evidence.evidence_dir.clone(),
            source_mutation: false,
            evidence_written: true,
            warnings: Vec::new(),
            blockers: Vec::new(),
            provider: effective_provider_name(&request).to_string(),
            llm_used: false,
        };
        write_report_files(&evidence, &report, None)?;
        return Ok(report);
    };

    save_json_pretty(&evidence.detection_json, &detected)?;
    let mut proposal = build_fix_proposal(&request, &target_root, &detected, &evidence)?;
    validate_patch_proposal(&mut proposal);
    save_json_pretty(&evidence.proposal_json, &proposal)?;

    let risk = classify_risk(&proposal);
    let mut blockers = Vec::new();
    let mut warnings = proposal.warnings.clone();
    if workspace_dirty {
        warnings.push("workspace_dirty:true".to_string());
    }
    if proposal.files_to_change.len() > request.max_files {
        blockers.push("max_files_exceeded".to_string());
    }
    if !risk_allowed(&request.risk_cap, &risk) {
        blockers.push("risk_cap_exceeded".to_string());
    }
    if !proposal.blockers.is_empty() {
        blockers.extend(proposal.blockers.clone());
    }

    let dry_run = simulate_fix(&target_root, &proposal);
    save_json_pretty(&evidence.dry_run_json, &dry_run)?;
    if !dry_run.blockers.is_empty() {
        blockers.extend(dry_run.blockers.clone());
    }
    blockers.sort();
    blockers.dedup();

    if !request.apply {
        let status = if blockers.is_empty() {
            FixStatus::ProposalCreated
        } else {
            FixStatus::DryRunFailed
        };
        let report = FixReport {
            fix_id: request.fix_id.clone(),
            target_path: target_root,
            mode: FixMode::DryRun,
            project_type,
            workspace_dirty,
            detected_problem: Some(problem_label(&detected.kind).to_string()),
            risk,
            files_planned: proposal.files_to_change.clone(),
            files_changed_by_patch: Vec::new(),
            files_changed_after_validation: Vec::new(),
            validation_side_effects: Vec::new(),
            validation_commands: detected.validation_commands.clone(),
            status,
            evidence_dir: evidence.evidence_dir.clone(),
            source_mutation: false,
            evidence_written: true,
            warnings,
            blockers,
            provider: proposal.proposer.clone(),
            llm_used: proposal.llm_used,
        };
        write_report_files(&evidence, &report, Some(&detected))?;
        return Ok(report);
    }

    if !blockers.is_empty() {
        let report = FixReport {
            fix_id: request.fix_id.clone(),
            target_path: target_root,
            mode: FixMode::Apply,
            project_type,
            workspace_dirty,
            detected_problem: Some(problem_label(&detected.kind).to_string()),
            risk,
            files_planned: proposal.files_to_change.clone(),
            files_changed_by_patch: Vec::new(),
            files_changed_after_validation: Vec::new(),
            validation_side_effects: Vec::new(),
            validation_commands: detected.validation_commands.clone(),
            status: FixStatus::Blocked,
            evidence_dir: evidence.evidence_dir.clone(),
            source_mutation: false,
            evidence_written: true,
            warnings,
            blockers,
            provider: proposal.proposer.clone(),
            llm_used: proposal.llm_used,
        };
        write_report_files(&evidence, &report, Some(&detected))?;
        return Ok(report);
    }

    let apply_result = apply_fix_proposal(&target_root, &proposal)?;
    if let Some(path) = &evidence.apply_result_json {
        save_json_pretty(path, &apply_result)?;
    }

    let git_status_before_validation = git_status_short(&target_root);
    let cargo_lock_before_validation = target_root.join("Cargo.lock").exists();
    let validation = run_fix_validation(&target_root, &evidence.evidence_dir, &detected);
    let git_status_after_validation = git_status_short(&target_root);
    let cargo_lock_after_validation = target_root.join("Cargo.lock").exists();
    let validation_side_effects = compute_validation_side_effects(
        &git_status_before_validation,
        &git_status_after_validation,
        &apply_result.files_changed,
        cargo_lock_before_validation,
        cargo_lock_after_validation,
    );
    if let Some(path) = &evidence.validation_json {
        save_json_pretty(path, &validation)?;
    }
    let status = match validation.status {
        AgentValidationStatus::Passed => FixStatus::ValidationPassed,
        AgentValidationStatus::Partial | AgentValidationStatus::Failed => {
            FixStatus::ValidationFailed
        }
        AgentValidationStatus::NotRun => FixStatus::Applied,
    };

    let report = FixReport {
        fix_id: request.fix_id.clone(),
        target_path: target_root,
        mode: FixMode::Apply,
        project_type,
        workspace_dirty,
        detected_problem: Some(problem_label(&detected.kind).to_string()),
        risk,
        files_planned: proposal.files_to_change.clone(),
        files_changed_by_patch: apply_result.files_changed.clone(),
        files_changed_after_validation: git_status_after_validation
            .iter()
            .map(|line| git_status_path(line))
            .collect(),
        validation_side_effects,
        validation_commands: detected.validation_commands.clone(),
        status,
        evidence_dir: evidence.evidence_dir.clone(),
        source_mutation: true,
        evidence_written: true,
        warnings,
        blockers: validation.blockers.clone(),
        provider: proposal.proposer.clone(),
        llm_used: proposal.llm_used,
    };
    write_report_files(&evidence, &report, Some(&detected))?;
    Ok(report)
}

pub fn print_fix(request: FixRequest) -> Result<String, String> {
    let apply_hint = display_rel(&request.target_path);
    let report = run_fix(request)?;
    let mode = match report.mode {
        FixMode::DryRun => "dry-run",
        FixMode::Apply => "apply",
    };
    let status = match report.status {
        FixStatus::ProposalCreated => "proposal_created",
        FixStatus::DryRunPassed => "dry_run_passed",
        FixStatus::DryRunFailed => "dry_run_failed",
        FixStatus::Applied => "applied",
        FixStatus::ValidationPassed => "validation_passed",
        FixStatus::ValidationFailed => "validation_failed",
        FixStatus::Detected => "detected",
        FixStatus::NoActionableProblem => "no_actionable_problem",
        FixStatus::Blocked => "blocked",
    };
    Ok(format!(
        "EVE Fix Report\n\nTarget: {}\nMode: {}\nProject type: {}\nWorkspace dirty: {}\nSource mutation: {}\nEvidence written: {}\n\nDetected problem:\n  {}\n\nProposed fix:\n  {}\n\nRisk:\n  {}\n\nFiles:\n  {}\n\nValidation plan:\n  {}\n\nStatus:\n  {}\n{}\n{}\n{}\n{}\n{}",
        display_rel(&report.target_path),
        mode,
        report.project_type,
        report.workspace_dirty,
        report.source_mutation,
        report.evidence_written,
        report.detected_problem.as_deref().unwrap_or("none"),
        if report.files_planned.is_empty() {
            "none".to_string()
        } else {
            report.files_planned.join(", ")
        },
        report.risk,
        if report.files_planned.is_empty() {
            "none".to_string()
        } else {
            report.files_planned.join(", ")
        },
        if report.validation_commands.is_empty() {
            "none".to_string()
        } else {
            report.validation_commands.join(", ")
        },
        status,
        if report.evidence_written {
            format!("\nEvidence:\n  {}", report.evidence_dir.display())
        } else {
            "\nEvidence:\n  none".to_string()
        },
        if report.warnings.is_empty() {
            String::new()
        } else {
            format!("\nWarnings:\n  {}", report.warnings.join("\n  "))
        },
        if report.validation_side_effects.is_empty() {
            String::new()
        } else {
            format!(
                "\nValidation side effects:\n  {}",
                report.validation_side_effects.join("\n  ")
            )
        },
        if report.blockers.is_empty() {
            String::new()
        } else {
            format!("\nBlockers:\n  {}", report.blockers.join("\n  "))
        },
        if matches!(report.mode, FixMode::DryRun) && matches!(report.status, FixStatus::ProposalCreated)
        {
            format!("\nNext:\n  cargo run -- fix {} --apply", apply_hint)
        } else {
            String::new()
        }
    ))
}

fn detect_problem(
    target_root: &Path,
    project_type: &str,
    workspace_dirty: bool,
    workspace_changes: Vec<String>,
    only: Option<FixOnly>,
) -> Result<Option<FixDetectedProblem>, String> {
    if project_type != "rust" {
        return Ok(None);
    }
    let categories = match only {
        Some(FixOnly::CargoCheck) => vec![FixProblemKind::CargoCheckFailure],
        Some(FixOnly::Ci) => vec![FixProblemKind::MissingCi],
        Some(FixOnly::Tests) => vec![FixProblemKind::MissingSmokeTest],
        Some(FixOnly::Docs) => vec![FixProblemKind::MissingReadmeValidation],
        None => vec![
            FixProblemKind::CargoCheckFailure,
            FixProblemKind::MissingCi,
            FixProblemKind::MissingSmokeTest,
            FixProblemKind::MissingReadmeValidation,
        ],
    };
    for category in categories {
        let detected = match category {
            FixProblemKind::CargoCheckFailure => {
                detect_cargo_check_failure(target_root, workspace_dirty, workspace_changes.clone())?
            }
            FixProblemKind::MissingCi => {
                detect_missing_ci(target_root, workspace_dirty, workspace_changes.clone())
            }
            FixProblemKind::MissingSmokeTest => {
                detect_missing_smoke_test(target_root, workspace_dirty, workspace_changes.clone())
            }
            FixProblemKind::MissingReadmeValidation => detect_missing_readme_validation(
                target_root,
                workspace_dirty,
                workspace_changes.clone(),
                only == Some(FixOnly::Docs),
            )?,
        };
        if detected.is_some() {
            return Ok(detected);
        }
    }
    Ok(None)
}

fn blocked_target_report(request: &FixRequest) -> Option<FixReport> {
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

fn blocked_report(request: &FixRequest, blocker: &str, target_path: PathBuf) -> FixReport {
    FixReport {
        fix_id: request.fix_id.clone(),
        target_path,
        mode: mode_for(request),
        project_type: "unknown".to_string(),
        workspace_dirty: false,
        detected_problem: None,
        risk: "low".to_string(),
        files_planned: Vec::new(),
        files_changed_by_patch: Vec::new(),
        files_changed_after_validation: Vec::new(),
        validation_side_effects: Vec::new(),
        validation_commands: Vec::new(),
        status: FixStatus::Blocked,
        evidence_dir: PathBuf::new(),
        source_mutation: false,
        evidence_written: false,
        warnings: Vec::new(),
        blockers: vec![blocker.to_string()],
        provider: "not_run".to_string(),
        llm_used: false,
    }
}

fn detect_cargo_check_failure(
    target_root: &Path,
    workspace_dirty: bool,
    workspace_changes: Vec<String>,
) -> Result<Option<FixDetectedProblem>, String> {
    let result = run_cargo(target_root.to_str().unwrap_or("."), &["check"]);
    if result.success {
        return Ok(None);
    }
    let output = format!("{}\n{}", result.stdout_tail, result.stderr_tail);
    let Some(module) = parse_missing_module_name(&output) else {
        return Ok(None);
    };
    Ok(Some(FixDetectedProblem {
        kind: FixProblemKind::CargoCheckFailure,
        description: format!("missing Rust module file for `{module}`"),
        goal: "Fix nearest Rust cargo check failure with minimal patch.".to_string(),
        project_type: "rust".to_string(),
        files_planned: vec![format!("src/{module}.rs")],
        validation_commands: rust_validation_commands(),
        workspace_dirty,
        workspace_changes,
        details: vec![output],
    }))
}

fn detect_missing_ci(
    target_root: &Path,
    workspace_dirty: bool,
    workspace_changes: Vec<String>,
) -> Option<FixDetectedProblem> {
    if target_root.join(".github/workflows/rust-ci.yml").exists() {
        return None;
    }
    Some(FixDetectedProblem {
        kind: FixProblemKind::MissingCi,
        description: "missing Rust CI workflow".to_string(),
        goal: "Add a minimal Rust CI workflow.".to_string(),
        project_type: "rust".to_string(),
        files_planned: vec![".github/workflows/rust-ci.yml".to_string()],
        validation_commands: vec![
            "cargo fmt --check".to_string(),
            "cargo check --all-targets".to_string(),
            "cargo test".to_string(),
        ],
        workspace_dirty,
        workspace_changes,
        details: Vec::new(),
    })
}

fn detect_missing_smoke_test(
    target_root: &Path,
    workspace_dirty: bool,
    workspace_changes: Vec<String>,
) -> Option<FixDetectedProblem> {
    if target_root.join("tests/eve_smoke.rs").exists() {
        return None;
    }
    Some(FixDetectedProblem {
        kind: FixProblemKind::MissingSmokeTest,
        description: "missing smoke test".to_string(),
        goal: "Add a minimal smoke test for the Rust project.".to_string(),
        project_type: "rust".to_string(),
        files_planned: vec!["tests/eve_smoke.rs".to_string()],
        validation_commands: rust_validation_commands(),
        workspace_dirty,
        workspace_changes,
        details: Vec::new(),
    })
}

fn detect_missing_readme_validation(
    target_root: &Path,
    workspace_dirty: bool,
    workspace_changes: Vec<String>,
    allow_create: bool,
) -> Result<Option<FixDetectedProblem>, String> {
    let readme = target_root.join("README.md");
    if !readme.exists() {
        if !allow_create {
            return Ok(None);
        }
        return Ok(Some(FixDetectedProblem {
            kind: FixProblemKind::MissingReadmeValidation,
            description: "missing README validation section".to_string(),
            goal: "Add README validation commands.".to_string(),
            project_type: "rust".to_string(),
            files_planned: vec!["README.md".to_string()],
            validation_commands: rust_validation_commands(),
            workspace_dirty,
            workspace_changes,
            details: vec!["README.md missing; create minimal validation section".to_string()],
        }));
    }
    let contents = fs::read_to_string(&readme)
        .map_err(|error| format!("read {}: {error}", readme.display()))?;
    if contents.contains("cargo fmt --check")
        && contents.contains("cargo check")
        && contents.contains("cargo test")
    {
        return Ok(None);
    }
    Ok(Some(FixDetectedProblem {
        kind: FixProblemKind::MissingReadmeValidation,
        description: "README lacks validation section".to_string(),
        goal: "Add README validation commands.".to_string(),
        project_type: "rust".to_string(),
        files_planned: vec!["README.md".to_string()],
        validation_commands: rust_validation_commands(),
        workspace_dirty,
        workspace_changes,
        details: Vec::new(),
    }))
}

fn build_fix_proposal(
    request: &FixRequest,
    target_root: &Path,
    detected: &FixDetectedProblem,
    evidence: &FixEvidencePaths,
) -> Result<PatchProposal, String> {
    let provider_name = effective_provider_name(request);
    if request.no_llm || provider_name == "rule_based" {
        return Ok(build_rule_based_fix_proposal(
            request,
            target_root,
            detected,
        ));
    }
    let provider = select_llm_provider_from_env();
    match build_llm_fix_proposal(request, detected, evidence, provider.as_ref()) {
        Ok(mut proposal) => {
            enforce_fix_scope(&mut proposal, &detected.files_planned);
            if proposal.status == ProposalStatus::Refused {
                let mut fallback = build_rule_based_fix_proposal(request, target_root, detected);
                fallback
                    .warnings
                    .push(format!("openai_fallback:{}", proposal.blockers.join("|")));
                Ok(fallback)
            } else {
                Ok(proposal)
            }
        }
        Err(reason) => {
            let mut fallback = build_rule_based_fix_proposal(request, target_root, detected);
            fallback.warnings.push(format!("openai_fallback:{reason}"));
            Ok(fallback)
        }
    }
}

fn build_llm_fix_proposal(
    request: &FixRequest,
    detected: &FixDetectedProblem,
    evidence: &FixEvidencePaths,
    provider: &dyn LlmProvider,
) -> Result<PatchProposal, String> {
    let request = LlmRequest {
        request_id: id("llm-fix"),
        purpose: LlmPurpose::ProposePatch,
        system_prompt: AGENT_SYSTEM_PROMPT.to_string(),
        input: format!(
            "Return JSON for one minimal safe repair proposal.\n\
problem_kind={}\n\
goal={}\n\
allowed_files={}\n\
allowed_ops=CreateFile,AppendFile,ReplaceFileIfExists,ReplaceExactText\n\
risk_cap={:?}\n\
No approval, no apply, no shell commands.",
            problem_label(&detected.kind),
            detected.goal,
            detected.files_planned.join(","),
            request.risk_cap
        ),
        expected_schema: PATCH_PROPOSAL_SCHEMA.to_string(),
        max_output_tokens: 1600,
        temperature: 0.0,
    };
    let response = provider.complete(&request)?;
    proposal_from_llm_response(
        evidence.evidence_dir.to_str().unwrap_or(".eva/fix"),
        "fix-task",
        "fix-plan",
        &detected.goal,
        &response,
    )
}

fn enforce_fix_scope(proposal: &mut PatchProposal, allowed_files: &[String]) {
    if proposal
        .files_to_change
        .iter()
        .any(|path| !allowed_files.contains(path))
        || proposal
            .patch_ops
            .iter()
            .any(|op| !allowed_files.contains(&op.path))
    {
        proposal.blockers.push("fix_scope_violation".to_string());
        proposal.status = ProposalStatus::Refused;
    }
}

fn build_rule_based_fix_proposal(
    request: &FixRequest,
    target_root: &Path,
    detected: &FixDetectedProblem,
) -> PatchProposal {
    let patch_ops = match detected.kind {
        FixProblemKind::CargoCheckFailure => vec![PatchOp {
            path: detected.files_planned[0].clone(),
            op: PatchOperationKind::CreateFile,
            description: "Create the missing module file with a minimal Rust stub.".to_string(),
            content: Some("// Generated by EVE fix\n".to_string()),
            find: None,
            replace: None,
        }],
        FixProblemKind::MissingCi => vec![PatchOp {
            path: ".github/workflows/rust-ci.yml".to_string(),
            op: PatchOperationKind::CreateFile,
            description: "Add minimal Rust CI workflow.".to_string(),
            content: Some(minimal_rust_ci_workflow()),
            find: None,
            replace: None,
        }],
        FixProblemKind::MissingSmokeTest => vec![PatchOp {
            path: "tests/eve_smoke.rs".to_string(),
            op: PatchOperationKind::CreateFile,
            description: "Add a minimal Rust smoke test.".to_string(),
            content: Some(
                "use std::path::Path;\n\n#[test]\nfn eve_smoke_cargo_toml_exists() {\n    assert!(Path::new(\"Cargo.toml\").exists());\n}\n"
                    .to_string(),
            ),
            find: None,
            replace: None,
        }],
        FixProblemKind::MissingReadmeValidation => {
            let readme = target_root.join("README.md");
            if readme.exists() {
                vec![PatchOp {
                    path: "README.md".to_string(),
                    op: PatchOperationKind::AppendFile,
                    description: "Append a validation section to README.".to_string(),
                    content: Some(readme_validation_section()),
                    find: None,
                    replace: None,
                }]
            } else {
                vec![PatchOp {
                    path: "README.md".to_string(),
                    op: PatchOperationKind::CreateFile,
                    description: "Create minimal README with validation section.".to_string(),
                    content: Some(format!("# {}\n{}", package_name(target_root), readme_validation_section())),
                    find: None,
                    replace: None,
                }]
            }
        }
    };
    PatchProposal {
        proposal_id: request.fix_id.clone(),
        task_id: request.fix_id.clone(),
        plan_id: request.fix_id.clone(),
        status: ProposalStatus::AwaitingApproval,
        created_at: now_unix(),
        updated_at: now_unix(),
        goal: detected.goal.clone(),
        summary: detected.description.clone(),
        proposer: "rule_based".to_string(),
        llm_used: false,
        files_to_change: patch_ops.iter().map(|op| op.path.clone()).collect(),
        forbidden_paths: vec![
            ".git/".to_string(),
            "target/".to_string(),
            "memory/".to_string(),
            "releases/".to_string(),
            "sandboxes/".to_string(),
            ".eva-runtime-tests/".to_string(),
            ".eva-evolution-tests/".to_string(),
        ],
        risk_level: classify_risk_from_count(patch_ops.len()).to_string(),
        approval_required: true,
        approved: false,
        approved_at: None,
        patch_ops,
        warnings: Vec::new(),
        blockers: Vec::new(),
    }
}

#[derive(serde::Serialize)]
struct FixDryRunResult {
    would_apply: bool,
    blockers: Vec<String>,
    files_changed: Vec<String>,
}

fn simulate_fix(target_root: &Path, proposal: &PatchProposal) -> FixDryRunResult {
    let mut blockers = Vec::new();
    for op in &proposal.patch_ops {
        if let Err(error) = validate_patch_path(&op.path) {
            blockers.push(error.to_string());
        }
        let path = target_root.join(&op.path);
        match op.op {
            PatchOperationKind::CreateFile if path.exists() => {
                blockers.push("create_file_exists".to_string());
            }
            PatchOperationKind::AppendFile if !path.exists() => {
                blockers.push("append_file_missing".to_string());
            }
            PatchOperationKind::ReplaceFileIfExists if !path.exists() => {
                blockers.push("replace_file_missing".to_string());
            }
            PatchOperationKind::ReplaceExactText => {
                let contents = fs::read_to_string(&path).unwrap_or_default();
                let find = op.find.as_deref().unwrap_or_default();
                let count = if find.is_empty() {
                    0
                } else {
                    contents.matches(find).count()
                };
                if count == 0 {
                    blockers.push("exact_text_not_found".to_string());
                } else if count > 1 {
                    blockers.push("ambiguous_exact_text_match".to_string());
                }
            }
            _ => {}
        }
    }
    blockers.sort();
    blockers.dedup();
    FixDryRunResult {
        would_apply: blockers.is_empty(),
        blockers,
        files_changed: proposal.files_to_change.clone(),
    }
}

#[derive(serde::Serialize)]
struct FixApplyResult {
    files_changed: Vec<String>,
    applied: bool,
}

fn apply_fix_proposal(
    target_root: &Path,
    proposal: &PatchProposal,
) -> Result<FixApplyResult, String> {
    let mut files_changed = Vec::new();
    for op in &proposal.patch_ops {
        validate_patch_path(&op.path).map_err(|error| error.to_string())?;
        let path = target_root.join(&op.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("create parent {}: {error}", parent.display()))?;
        }
        match op.op {
            PatchOperationKind::CreateFile => {
                if path.exists() {
                    return Err("create_file_exists".to_string());
                }
                fs::write(&path, op.content.clone().unwrap_or_default())
                    .map_err(|error| format!("write {}: {error}", path.display()))?;
            }
            PatchOperationKind::AppendFile => {
                use std::io::Write;
                if !path.exists() {
                    return Err("append_file_missing".to_string());
                }
                let mut file = fs::OpenOptions::new()
                    .append(true)
                    .open(&path)
                    .map_err(|error| format!("open append {}: {error}", path.display()))?;
                file.write_all(op.content.clone().unwrap_or_default().as_bytes())
                    .map_err(|error| format!("append {}: {error}", path.display()))?;
            }
            PatchOperationKind::ReplaceFileIfExists => {
                if !path.exists() {
                    return Err("replace_file_missing".to_string());
                }
                fs::write(&path, op.content.clone().unwrap_or_default())
                    .map_err(|error| format!("replace {}: {error}", path.display()))?;
            }
            PatchOperationKind::ReplaceExactText => {
                let contents = fs::read_to_string(&path)
                    .map_err(|error| format!("read replace {}: {error}", path.display()))?;
                let find = op.find.as_deref().unwrap_or_default();
                let count = contents.matches(find).count();
                if count == 0 {
                    return Err("exact_text_not_found".to_string());
                }
                if count > 1 {
                    return Err("ambiguous_exact_text_match".to_string());
                }
                let next = contents.replace(find, op.replace.as_deref().unwrap_or_default());
                fs::write(&path, next)
                    .map_err(|error| format!("write replace {}: {error}", path.display()))?;
            }
        }
        files_changed.push(op.path.clone());
    }
    Ok(FixApplyResult {
        files_changed,
        applied: true,
    })
}

fn run_fix_validation(
    target_root: &Path,
    _evidence_dir: &Path,
    detected: &FixDetectedProblem,
) -> ValidationRun {
    let commands = match detected.kind {
        FixProblemKind::MissingCi => vec![
            run_cargo(target_root.to_str().unwrap_or("."), &["fmt", "--check"]),
            run_cargo(
                target_root.to_str().unwrap_or("."),
                &["check", "--all-targets"],
            ),
            run_cargo(target_root.to_str().unwrap_or("."), &["test"]),
        ],
        _ => vec![
            run_cargo(target_root.to_str().unwrap_or("."), &["fmt", "--check"]),
            run_cargo(target_root.to_str().unwrap_or("."), &["check"]),
            run_cargo(target_root.to_str().unwrap_or("."), &["test"]),
        ],
    };
    let status = if commands.iter().all(|cmd| cmd.success) {
        AgentValidationStatus::Passed
    } else if commands.iter().any(|cmd| cmd.success) {
        AgentValidationStatus::Partial
    } else {
        AgentValidationStatus::Failed
    };
    ValidationRun {
        validation_id: id("fix-validation"),
        task_id: None,
        proposal_id: None,
        status,
        started_at: now_unix(),
        finished_at: now_unix(),
        commands,
        warnings: Vec::new(),
        blockers: Vec::new(),
    }
}

fn build_evidence_paths(request: &FixRequest, target_root: &Path) -> FixEvidencePaths {
    let evidence_root = if request.evidence_dir.is_absolute() {
        request.evidence_dir.clone()
    } else {
        target_root.join(&request.evidence_dir)
    };
    let evidence_dir = evidence_root.join(&request.fix_id);
    FixEvidencePaths {
        request_json: evidence_dir.join("request.json"),
        detection_json: evidence_dir.join("detection.json"),
        proposal_json: evidence_dir.join("proposal.json"),
        dry_run_json: evidence_dir.join("dry_run.json"),
        apply_result_json: request
            .apply
            .then(|| evidence_dir.join("apply_result.json")),
        validation_json: request.apply.then(|| evidence_dir.join("validation.json")),
        report_md: evidence_dir.join("report.md"),
        report_json: evidence_dir.join("report.json"),
        evidence_dir,
    }
}

fn write_report_files(
    evidence: &FixEvidencePaths,
    report: &FixReport,
    detected: Option<&FixDetectedProblem>,
) -> Result<(), String> {
    save_json_pretty(&evidence.report_json, report)?;
    let mut markdown = String::from("# EVE Fix Report\n\n");
    markdown.push_str(&format!("- fix_id: `{}`\n", report.fix_id));
    markdown.push_str(&format!(
        "- target: `{}`\n",
        display_rel(&report.target_path)
    ));
    markdown.push_str(&format!("- mode: `{:?}`\n", report.mode));
    markdown.push_str(&format!("- status: `{:?}`\n", report.status));
    markdown.push_str(&format!("- project_type: `{}`\n", report.project_type));
    markdown.push_str(&format!(
        "- workspace_dirty: `{}`\n",
        report.workspace_dirty
    ));
    if let Some(problem) = &report.detected_problem {
        markdown.push_str(&format!("- detected_problem: `{problem}`\n"));
    }
    if let Some(detected) = detected {
        markdown.push_str(&format!("\n## Problem\n\n{}\n", detected.description));
    }
    if !report.files_planned.is_empty() {
        markdown.push_str("\n## Files Planned\n\n");
        for file in &report.files_planned {
            markdown.push_str(&format!("- `{file}`\n"));
        }
    }
    if !report.files_changed_by_patch.is_empty() {
        markdown.push_str("\n## Files Changed By Patch\n\n");
        for file in &report.files_changed_by_patch {
            markdown.push_str(&format!("- `{file}`\n"));
        }
    }
    if !report.files_changed_after_validation.is_empty() {
        markdown.push_str("\n## Files Changed After Validation\n\n");
        for file in &report.files_changed_after_validation {
            markdown.push_str(&format!("- `{file}`\n"));
        }
    }
    if !report.validation_side_effects.is_empty() {
        markdown.push_str("\n## Validation Side Effects\n\n");
        for file in &report.validation_side_effects {
            markdown.push_str(&format!("- `{file}`\n"));
        }
    }
    if !report.validation_commands.is_empty() {
        markdown.push_str("\n## Validation\n\n");
        for command in &report.validation_commands {
            markdown.push_str(&format!("- `{command}`\n"));
        }
    }
    if !report.warnings.is_empty() {
        markdown.push_str("\n## Warnings\n\n");
        for item in &report.warnings {
            markdown.push_str(&format!("- `{item}`\n"));
        }
    }
    if !report.blockers.is_empty() {
        markdown.push_str("\n## Blockers\n\n");
        for item in &report.blockers {
            markdown.push_str(&format!("- `{item}`\n"));
        }
    }
    if let Some(parent) = evidence.report_md.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
    }
    fs::write(&evidence.report_md, markdown)
        .map_err(|error| format!("write {}: {error}", evidence.report_md.display()))
}

fn minimal_rust_ci_workflow() -> String {
    "name: rust-ci\n\non:\n  push:\n  pull_request:\n\njobs:\n  rust:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - uses: dtolnay/rust-toolchain@stable\n      - run: cargo fmt --check\n      - run: cargo check --all-targets\n      - run: cargo test\n".to_string()
}

fn readme_validation_section() -> String {
    "\n## Validation\n\n```bash\ncargo fmt --check\ncargo check\ncargo test\n```\n".to_string()
}

fn rust_validation_commands() -> Vec<String> {
    vec![
        "cargo fmt --check".to_string(),
        "cargo check".to_string(),
        "cargo test".to_string(),
    ]
}

fn effective_provider_name(request: &FixRequest) -> &'static str {
    if request.no_llm {
        "rule_based"
    } else if let Some(provider) = &request.provider {
        if provider == "openai" {
            selected_llm_provider_name_from_env()
        } else {
            "rule_based"
        }
    } else {
        selected_llm_provider_name_from_env()
    }
}

fn classify_risk(proposal: &PatchProposal) -> String {
    classify_risk_from_count(proposal.files_to_change.len()).to_string()
}

fn classify_risk_from_count(file_count: usize) -> &'static str {
    if file_count <= 1 {
        "low"
    } else {
        "medium"
    }
}

fn risk_allowed(cap: &FixRiskCap, risk: &str) -> bool {
    matches!(
        (cap, risk),
        (FixRiskCap::Low, "low") | (FixRiskCap::Medium, "low" | "medium")
    )
}

fn problem_label(kind: &FixProblemKind) -> &'static str {
    match kind {
        FixProblemKind::CargoCheckFailure => "cargo_check_failure",
        FixProblemKind::MissingCi => "missing_ci",
        FixProblemKind::MissingSmokeTest => "missing_smoke_test",
        FixProblemKind::MissingReadmeValidation => "missing_readme_validation",
    }
}

fn package_name(target_root: &Path) -> String {
    let cargo = target_root.join("Cargo.toml");
    let contents = fs::read_to_string(cargo).unwrap_or_default();
    contents
        .lines()
        .find_map(|line| {
            line.split_once('=').and_then(|(left, right)| {
                if left.trim() == "name" {
                    Some(right.trim().trim_matches('"').to_string())
                } else {
                    None
                }
            })
        })
        .unwrap_or_else(|| "Project".to_string())
}

fn parse_missing_module_name(output: &str) -> Option<String> {
    let marker = "file not found for module `";
    let start = output.find(marker)? + marker.len();
    let rest = &output[start..];
    let end = rest.find('`')?;
    Some(rest[..end].to_string())
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

fn git_status_path(line: &str) -> String {
    line.get(3..)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(line)
        .to_string()
}

fn compute_validation_side_effects(
    before_validation: &[String],
    after_validation: &[String],
    patch_files: &[String],
    cargo_lock_before_validation: bool,
    cargo_lock_after_validation: bool,
) -> Vec<String> {
    let before_paths: std::collections::BTreeSet<String> = before_validation
        .iter()
        .map(|line| git_status_path(line))
        .collect();
    let patch_paths: std::collections::BTreeSet<String> = patch_files.iter().cloned().collect();
    let mut side_effects = Vec::new();
    for path in after_validation.iter().map(|line| git_status_path(line)) {
        if !before_paths.contains(&path) && !patch_paths.contains(&path) {
            side_effects.push(path);
        }
    }
    if !cargo_lock_before_validation
        && cargo_lock_after_validation
        && !side_effects.iter().any(|path| path == "Cargo.lock")
    {
        side_effects.push("Cargo.lock".to_string());
    }
    side_effects.sort();
    side_effects.dedup();
    side_effects
}

fn mode_for(request: &FixRequest) -> FixMode {
    if request.apply {
        FixMode::Apply
    } else {
        FixMode::DryRun
    }
}

fn display_rel(path: &Path) -> String {
    path.to_string_lossy().to_string()
}
