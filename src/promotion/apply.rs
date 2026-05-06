use std::process::Command;

use crate::contracts::SandboxResult;
use crate::evolution::{
    dedup, memory, mutator, refresh_report, scorer, update_portfolio_after_log,
    update_portfolio_after_replay,
};
use crate::promotion::gate::check_promotion_gate;
use crate::promotion::review::review_candidate;
use crate::sandbox::{manager, runner, snapshot};

#[derive(Debug, Clone, PartialEq, Eq)]
enum PromotionBackupKind {
    ExistingFile { original_contents: String },
    NewFile,
}

pub fn list_candidates(memory_root: &str) -> Result<String, String> {
    let summaries = memory::list_candidate_summaries(memory_root)?;
    if summaries.is_empty() {
        return Ok("(none)".to_string());
    }
    Ok(summaries
        .iter()
        .map(|summary| {
            let report_path = std::path::Path::new(memory_root)
                .join("reports")
                .join(format!("{}.ru.md", summary.run_id));
            let review = review_candidate(".", memory_root, &summary.run_id).ok();
            let replay_status = review
                .as_ref()
                .map(|review| review.replay_status.as_str())
                .unwrap_or("not_run");
            let promotion_ready = review
                .as_ref()
                .map(|review| review.promotion_allowed)
                .unwrap_or(false);
            let blocker_reason = review
                .as_ref()
                .and_then(|review| review.promotion_ready_reason.clone())
                .unwrap_or_else(|| "ready".to_string());
            format!(
                "{} score={:.1} risk={:.2} kind={} useful={} replay_status={} promotion_ready={} reason={} target={} report={}",
                summary.run_id,
                summary.score,
                summary.risk,
                summary.mutation_kind,
                summary.useful_change,
                replay_status,
                promotion_ready,
                blocker_reason,
                summary.target_file,
                if report_path.exists() {
                    report_path.display().to_string()
                } else {
                    "(none)".to_string()
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

pub fn replay_candidate(project_root: &str, memory_root: &str, run_id: &str) -> Result<(), String> {
    let mutation = memory::load_candidate(memory_root, run_id)?;
    let summary = memory::load_candidate_summary(memory_root, run_id)?;
    let sandbox_path = manager::create_sandbox_path();
    snapshot::copy_project(project_root, &sandbox_path)?;
    let result = replay_in_sandbox(&sandbox_path, &mutation);
    let cleanup = manager::destroy_sandbox(&sandbox_path);
    let sandbox_destroyed = cleanup.is_ok();
    let sandbox = result?;
    cleanup?;

    let test_ref = sandbox
        .test
        .as_ref()
        .ok_or_else(|| "replay did not run cargo test".to_string())?;
    let score = scorer::score_cycle(
        mutation.kind,
        &sandbox.check,
        test_ref,
        sandbox.run.as_ref(),
    );
    let stdout = memory::combined_stdout(&sandbox);
    let stderr = memory::combined_stderr(&sandbox);
    let replay = memory::ReplayResult {
        run_id: run_id.to_string(),
        replay_status: if score.score >= memory::CANDIDATE_THRESHOLD
            && score.accepted
            && score.useful_change
        {
            crate::contracts::EvolutionStatus::Candidate
        } else if score.accepted {
            crate::contracts::EvolutionStatus::Passed
        } else {
            crate::contracts::EvolutionStatus::Failed
        },
        matches_stored_summary: (score.score - summary.score).abs() < f32::EPSILON
            && score.check_passed == summary.cargo_check_ok
            && score.test_passed == summary.cargo_test_ok
            && score.run_passed == summary.cargo_run_ok,
        score: score.score,
        cargo_check_ok: score.check_passed,
        cargo_test_ok: score.test_passed,
        cargo_run_ok: score.run_passed,
        stdout_digest: crate::contracts::sha256_digest(&stdout),
        stderr_digest: crate::contracts::sha256_digest(&stderr),
        stderr_tail: crate::contracts::tail(&stderr, 1200),
        sandbox_destroyed,
        timestamp_unix: memory::now_unix(),
    };
    crate::evolution::metrics::update_metrics_after_replay(memory_root, &replay)?;
    memory::store_replay_result(memory_root, run_id, &replay)?;
    update_portfolio_after_replay(memory_root, mutation.kind, &replay)?;
    let _ = crate::evolution::refresh_strategy_portfolio(memory_root);
    refresh_report(memory_root, run_id)?;
    Ok(())
}

pub fn promote_candidate(
    project_root: &str,
    memory_root: &str,
    run_id: &str,
) -> Result<(), String> {
    let mutation = memory::load_candidate(memory_root, run_id)?;
    let summary = memory::load_candidate_summary(memory_root, run_id)?;
    let decision = check_promotion_gate(&mutation, summary.score);
    if !decision.allowed {
        return Err(decision.reason);
    }

    let target_path = std::path::Path::new(project_root).join(&mutation.target_file);
    let backup_kind = prepare_promotion_backup(&target_path)?;

    mutator::apply_mutation(project_root, &mutation)?;
    let validation = validate_promoted_project(project_root);
    let (check, test) = match validation {
        Ok(result) => result,
        Err(error) => {
            rollback_promoted_target(&target_path, &backup_kind)
                .map_err(|restore_error| format!("{error}; restore failed: {restore_error}"))?;
            return Err(error);
        }
    };
    let sandbox = SandboxResult {
        sandbox_path: project_root.to_string(),
        check,
        test: Some(test),
        run: None,
    };
    let test_ref = sandbox
        .test
        .as_ref()
        .ok_or_else(|| "promotion did not run cargo test".to_string())?;
    let score = scorer::score_cycle(mutation.kind, &sandbox.check, test_ref, None);
    if !score.accepted {
        return Err("promotion validation failed".to_string());
    }
    let entry = memory::build_log_entry(
        memory::new_run_id(),
        &mutation,
        dedup::compute_mutation_digest(&mutation),
        &score,
        &sandbox,
        true,
        false,
    );
    memory::append_jsonl(
        std::path::Path::new(memory_root).join("evolution.jsonl"),
        &entry,
    )?;
    crate::evolution::metrics::update_metrics_after_log(memory_root, &entry)?;
    update_portfolio_after_log(memory_root, &entry)?;
    let _ = crate::evolution::refresh_strategy_portfolio(memory_root);
    Ok(())
}

fn prepare_promotion_backup(target_path: &std::path::Path) -> Result<PromotionBackupKind, String> {
    if target_path.exists() {
        let original_contents = std::fs::read_to_string(target_path)
            .map_err(|error| format!("failed to backup promotion target: {error}"))?;
        return Ok(PromotionBackupKind::ExistingFile { original_contents });
    }

    if let Some(parent) = target_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create promotion target parent: {error}"))?;
    }

    Ok(PromotionBackupKind::NewFile)
}

fn rollback_promoted_target(
    target_path: &std::path::Path,
    backup_kind: &PromotionBackupKind,
) -> Result<(), String> {
    match backup_kind {
        PromotionBackupKind::ExistingFile { original_contents } => {
            std::fs::write(target_path, original_contents)
                .map_err(|error| format!("failed to restore promotion target: {error}"))
        }
        PromotionBackupKind::NewFile => {
            if target_path.exists() {
                std::fs::remove_file(target_path)
                    .map_err(|error| format!("failed to remove promoted file: {error}"))?;
            }
            Ok(())
        }
    }
}

fn validate_promoted_project(
    project_root: &str,
) -> Result<
    (
        crate::contracts::CommandResult,
        crate::contracts::CommandResult,
    ),
    String,
> {
    run_host_command(project_root, "cargo", &["fmt"])?;
    let check = run_host_command(project_root, "cargo", &["check"])?;
    let test = run_host_command(project_root, "cargo", &["test"])?;
    Ok((check, test))
}

fn replay_in_sandbox(
    sandbox_path: &str,
    mutation: &crate::contracts::MutationContract,
) -> Result<SandboxResult, String> {
    mutator::apply_mutation(sandbox_path, mutation)?;
    let check = runner::run_cargo_check(sandbox_path);
    let test = if check.success {
        Some(runner::run_cargo_test(sandbox_path))
    } else {
        None
    };
    let run = if test.as_ref().is_some_and(|result| result.success) {
        Some(runner::run_cargo_run(sandbox_path))
    } else {
        None
    };
    Ok(SandboxResult {
        sandbox_path: sandbox_path.to_string(),
        check,
        test,
        run,
    })
}

fn run_host_command(
    project_root: &str,
    bin: &str,
    args: &[&str],
) -> Result<crate::contracts::CommandResult, String> {
    let start = std::time::Instant::now();
    let output = Command::new(bin)
        .args(args)
        .current_dir(project_root)
        .output()
        .map_err(|error| format!("failed to run {bin}: {error}"))?;
    let result = crate::contracts::CommandResult {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        duration_ms: start.elapsed().as_millis(),
    };
    if result.success {
        Ok(result)
    } else {
        Err(format!(
            "{bin} {} failed: {}",
            args.join(" "),
            result.stderr
        ))
    }
}
