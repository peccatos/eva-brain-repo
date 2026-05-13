use eva_runtime_with_task_validator::{
    adjust_task_from_campaign, approval_log, approval_status, approve_candidate, autonomy_status,
    build_evidence_bundle, build_external_patch_package, build_pr_package,
    build_project_phase_runtime_output, build_recovery_manifest, build_self_review_package,
    build_workspace_snapshot, candidate_diff, candidate_lifecycle, default_corpus_contract,
    defer_candidate, distill_patterns, fix_generated_test_names, governance_status, ingest_corpus,
    ingest_repo_patterns, latest_corpus_id, learning_summary, list_adjusted_tasks,
    list_bounded_runs, list_candidates, list_corpora, list_evidence_bundles,
    list_external_patch_packages, list_pr_packages, list_recovery_manifests, list_releases,
    list_self_review_packages, list_suggested_tasks, list_supervised_runs,
    list_workspace_snapshots, load_corpus_summary, load_metrics, preview_campaign_recombination,
    print_artifact_audit, print_artifact_audit_json, print_benchmark, print_bounded_run_report,
    print_campaign, print_campaign_report, print_capability_policy, print_determinism_audit,
    print_determinism_audit_json, print_eva_status, print_evolution_policy, print_final_rc_report,
    print_future_phases, print_future_phases_json, print_hygiene_plan, print_hygiene_report,
    print_last_bounded_run, print_last_campaign_report, print_last_evidence_bundle,
    print_last_external_patch_package, print_last_pr_package, print_last_recovery_manifest,
    print_last_release, print_last_report, print_last_self_review_package,
    print_last_supervised_run, print_last_task_adjustment, print_last_workspace_snapshot,
    print_operator_console, print_operator_runbook, print_ops_json, print_ops_status,
    print_portfolio, print_preflight_gate, print_preflight_gate_json, print_preflight_gate_v3,
    print_promotion_queue, print_proof_json, print_proof_report, print_proof_snapshot,
    print_proof_snapshot_json, print_quality_report, print_record_release_attempt,
    print_release_approve, print_release_bundle_json, print_release_changelog,
    print_release_health, print_release_health_json, print_release_ledger,
    print_release_ledger_json, print_release_manifest, print_release_preflight_json,
    print_release_proposal, print_release_proposal_json, print_release_status, print_report,
    print_rollback_manifest, print_runtime_candidate, print_runtime_cli_contract,
    print_runtime_service, print_runtime_validation, print_strategy_portfolio,
    print_supervised_run_report, print_trust_decision, print_trust_proof_report,
    promote_approved_candidate, promote_candidate, promotion_blocked_items,
    promotion_ready_approved, promotion_ready_items, refresh_metrics, refresh_portfolio,
    refresh_promotion_queue, refresh_report, refresh_strategy_portfolio, reject_candidate,
    render_plans, render_recombined_hypotheses, replay_candidate, review_candidate, run_benchmark,
    run_bounded_evolution, run_demo, run_evolution_cycle, run_planned_cycles,
    run_planned_evolution_cycle, run_recombined_evolution_cycle, run_repo_patch_report,
    run_stored_campaign, run_task_from_path, run_tui, serve_runtime_daemon,
    should_run_repo_patch_mode, suggest_strategy_tasks, supervise_task, CycleInput,
    RepoPatchCliConfig, RuntimeCliCommand, RuntimeCycleRunner, RUNTIME_CLI_HELP,
};
use serde::Deserialize;
use std::fs;
use std::path::Path;

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if should_run_repo_patch_mode(args.iter().map(String::as_str)) {
        match RepoPatchCliConfig::parse_from_iter(args) {
            Ok(config) => match run_repo_patch_report(&config) {
                Ok(execution) => println!("{}", execution.stdout_output()),
                Err(err) => {
                    eprintln!("repo_patch_error: {err}");
                    std::process::exit(1);
                }
            },
            Err(err) => {
                eprintln!("repo_patch_cli_error: {err}");
                std::process::exit(1);
            }
        }
        return;
    }

    match RuntimeCliCommand::parse_from_iter(args) {
        Ok(RuntimeCliCommand::Help) => {
            println!("{RUNTIME_CLI_HELP}");
            return;
        }
        Ok(RuntimeCliCommand::Once) => {}
        Ok(RuntimeCliCommand::Tui) => {
            match run_tui(".", "memory") {
                Ok(output) => println!("{output}"),
                Err(err) => {
                    eprintln!("tui_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::Status) => {
            match print_runtime_validation(".", "memory") {
                Ok(output) => println!("{output}"),
                Err(err) => {
                    eprintln!("status_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::Evolve) => {
            if let Err(err) = run_evolution_cycle(".") {
                eprintln!("evolution_cycle_error: {err}");
                std::process::exit(1);
            }
            println!("evolution_cycle_status: ok");
            return;
        }
        Ok(RuntimeCliCommand::PlanEvolution) => {
            match render_plans("memory") {
                Ok(output) => println!("{output}"),
                Err(err) => {
                    eprintln!("plan_evolution_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::EvolvePlanned) => {
            if let Err(err) = run_planned_evolution_cycle(".", "memory") {
                eprintln!("planned_evolution_error: {err}");
                std::process::exit(1);
            }
            println!("planned_evolution_status: ok");
            return;
        }
        Ok(RuntimeCliCommand::EvolvePlannedN(count)) => {
            match run_planned_cycles(".", "memory", count) {
                Ok(run_ids) => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&run_ids).expect("serialize run ids")
                    )
                }
                Err(err) => {
                    eprintln!("planned_evolution_n_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::EvolutionBenchmark(count)) => {
            match run_benchmark(".", "memory", count) {
                Ok(benchmark) => println!("{}", print_benchmark(&benchmark)),
                Err(err) => {
                    eprintln!("evolution_benchmark_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::AutonomyStatus) => {
            match autonomy_status(".", "memory") {
                Ok(status) => println!(
                    "{}",
                    serde_json::to_string_pretty(&status).expect("serialize autonomy status")
                ),
                Err(err) => {
                    eprintln!("autonomy_status_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::Metrics) => {
            match load_metrics("memory") {
                Ok(metrics) => println!(
                    "{}",
                    serde_json::to_string_pretty(&metrics).expect("serialize metrics")
                ),
                Err(err) => {
                    eprintln!("metrics_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::MetricsRefresh) => {
            match refresh_metrics("memory") {
                Ok(metrics) => println!(
                    "{}",
                    serde_json::to_string_pretty(&metrics).expect("serialize metrics")
                ),
                Err(err) => {
                    eprintln!("metrics_refresh_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::Portfolio) => {
            match print_portfolio("memory") {
                Ok(summary) => println!("{summary}"),
                Err(err) => {
                    eprintln!("portfolio_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::PortfolioRefresh) => {
            match refresh_portfolio("memory") {
                Ok(portfolio) => println!(
                    "{}",
                    serde_json::to_string_pretty(&portfolio).expect("serialize portfolio")
                ),
                Err(err) => {
                    eprintln!("portfolio_refresh_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::StrategyPortfolio) => {
            match print_strategy_portfolio("memory") {
                Ok(summary) => println!("{summary}"),
                Err(err) => {
                    eprintln!("strategy_portfolio_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::StrategyPortfolioRefresh) => {
            match refresh_strategy_portfolio("memory") {
                Ok(portfolio) => println!(
                    "{}",
                    serde_json::to_string_pretty(&portfolio).expect("serialize strategy portfolio")
                ),
                Err(err) => {
                    eprintln!("strategy_portfolio_refresh_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::EvolutionPolicy) => {
            match print_evolution_policy(".", "memory", None) {
                Ok(policy) => println!("{policy}"),
                Err(err) => {
                    eprintln!("evolution_policy_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::QualityReport(run_id)) => {
            match print_quality_report("memory", &run_id) {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("quality_report_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::EvolutionHygiene) => {
            match print_hygiene_report(".", "memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("evolution_hygiene_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::HygienePlan) => {
            match print_hygiene_plan(".", "memory") {
                Ok(plan) => println!("{plan}"),
                Err(err) => {
                    eprintln!("hygiene_plan_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::HygieneFixGeneratedTests) => {
            match fix_generated_test_names(".") {
                Ok(status) => println!("{status}"),
                Err(err) => {
                    eprintln!("hygiene_fix_generated_tests_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::IngestCorpus(path)) => {
            match ingest_corpus("memory", &default_corpus_contract(&path)) {
                Ok(summary) => println!(
                    "{}",
                    serde_json::to_string_pretty(&summary).expect("serialize corpus summary")
                ),
                Err(err) => {
                    eprintln!("ingest_corpus_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::IngestCorpusContract(path)) => {
            let contract = fs::read_to_string(&path)
                .map_err(|error| format!("failed to read corpus contract: {error}"))
                .and_then(|contents| {
                    serde_json::from_str::<eva_runtime_with_task_validator::CorpusIngestContract>(
                        &contents,
                    )
                    .map_err(|error| format!("failed to parse corpus contract: {error}"))
                });
            match contract.and_then(|contract| ingest_corpus("memory", &contract)) {
                Ok(summary) => println!(
                    "{}",
                    serde_json::to_string_pretty(&summary).expect("serialize corpus summary")
                ),
                Err(err) => {
                    eprintln!("ingest_corpus_contract_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::CorpusSummary(corpus_id)) => {
            let resolved_id = resolve_corpus_alias("memory", &corpus_id);
            match resolved_id.and_then(|resolved| load_corpus_summary("memory", &resolved)) {
                Ok(summary) => println!(
                    "{}",
                    serde_json::to_string_pretty(&summary).expect("serialize corpus summary")
                ),
                Err(err) => {
                    eprintln!("corpus_summary_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ListCorpora) => {
            match list_corpora("memory") {
                Ok(corpora) => println!("{}", render_corpora_listing("memory", &corpora)),
                Err(err) => {
                    eprintln!("list_corpora_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::SuggestStrategyTasks(corpus_id)) => {
            let resolved_id = resolve_corpus_alias("memory", &corpus_id);
            match resolved_id.and_then(|resolved| suggest_strategy_tasks("memory", &resolved)) {
                Ok(tasks) => println!(
                    "{}",
                    serde_json::to_string_pretty(&tasks).expect("serialize suggested tasks")
                ),
                Err(err) => {
                    eprintln!("suggest_strategy_tasks_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ListSuggestedTasks) => {
            match list_suggested_tasks("memory") {
                Ok(tasks) => println!("{}", tasks.join("\n")),
                Err(err) => {
                    eprintln!("list_suggested_tasks_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::LearningSummary) => {
            match learning_summary("memory") {
                Ok(summary) => println!("{summary}"),
                Err(err) => {
                    eprintln!("learning_summary_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::LastReport) => {
            match print_last_report("memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("last_report_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::Report(run_id)) => {
            match print_report("memory", &run_id) {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("report_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ReportRefresh(run_id)) => {
            match refresh_report("memory", &run_id) {
                Ok(report) => println!(
                    "{}",
                    serde_json::to_string_pretty(&report).expect("serialize refreshed report")
                ),
                Err(err) => {
                    eprintln!("report_refresh_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ReviewCandidate(run_id)) => {
            match review_candidate(".", "memory", &run_id) {
                Ok(review) => println!(
                    "{}",
                    serde_json::to_string_pretty(&review).expect("serialize candidate review")
                ),
                Err(err) => {
                    eprintln!("review_candidate_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::CandidateDiff(run_id)) => {
            match candidate_diff("memory", &run_id) {
                Ok(diff) => println!("{diff}"),
                Err(err) => {
                    eprintln!("candidate_diff_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ListCandidates) => {
            match list_candidates("memory") {
                Ok(output) => println!("{output}"),
                Err(err) => {
                    eprintln!("list_candidates_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::RunTask(path)) => {
            match run_task_from_path(".", "memory", &path) {
                Ok(campaign) => println!("{}", print_campaign(&campaign)),
                Err(err) => {
                    eprintln!("run_task_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::Campaign(task_id)) => {
            match run_stored_campaign(".", "memory", &task_id) {
                Ok(campaign) => println!("{}", print_campaign(&campaign)),
                Err(err) => {
                    eprintln!("campaign_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::LastCampaignReport) => {
            match print_last_campaign_report("memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("last_campaign_report_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::CampaignReport(campaign_id)) => {
            match print_campaign_report("memory", &campaign_id) {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("campaign_report_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::AdjustTaskFromCampaign(campaign_id)) => {
            match adjust_task_from_campaign("memory", &campaign_id) {
                Ok(adjustment) => println!(
                    "{}",
                    serde_json::to_string_pretty(&adjustment).expect("serialize task adjustment")
                ),
                Err(err) => {
                    eprintln!("adjust_task_from_campaign_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::LastTaskAdjustment) => {
            match print_last_task_adjustment("memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("last_task_adjustment_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ListAdjustedTasks) => {
            match list_adjusted_tasks("memory") {
                Ok(tasks) => println!("{}", tasks.join("\n")),
                Err(err) => {
                    eprintln!("list_adjusted_tasks_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::CampaignRecombinePreview(task_path)) => {
            match eva_runtime_with_task_validator::evolution::load_task_contract(Path::new(
                &task_path,
            ))
            .and_then(|task| preview_campaign_recombination("memory", &task))
            {
                Ok(preview) => println!(
                    "{}",
                    serde_json::to_string_pretty(&preview)
                        .expect("serialize campaign recombine preview")
                ),
                Err(err) => {
                    eprintln!("campaign_recombine_preview_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::EvolveBounded { task_path, cycles }) => {
            match run_bounded_evolution(".", "memory", &task_path, cycles) {
                Ok(summary) => println!(
                    "{}",
                    serde_json::to_string_pretty(&summary).expect("serialize bounded summary")
                ),
                Err(err) => {
                    eprintln!("evolve_bounded_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::LastBoundedRun) => {
            match print_last_bounded_run("memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("last_bounded_run_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::BoundedRunReport(bounded_run_id)) => {
            match print_bounded_run_report("memory", &bounded_run_id) {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("bounded_run_report_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ListBoundedRuns) => {
            match list_bounded_runs("memory") {
                Ok(runs) => println!("{}", runs.join("\n")),
                Err(err) => {
                    eprintln!("list_bounded_runs_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::RefreshPromotionQueue) => {
            match refresh_promotion_queue(".", "memory") {
                Ok(queue) => println!(
                    "{}",
                    serde_json::to_string_pretty(&queue).expect("serialize promotion queue")
                ),
                Err(err) => {
                    eprintln!("refresh_promotion_queue_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::PromotionQueue) => {
            match print_promotion_queue(".", "memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("promotion_queue_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::PromotionReady) => {
            match promotion_ready_items(".", "memory") {
                Ok(items) => println!(
                    "{}",
                    serde_json::to_string_pretty(&items).expect("serialize promotion ready items")
                ),
                Err(err) => {
                    eprintln!("promotion_ready_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::PromotionBlocked) => {
            match promotion_blocked_items(".", "memory") {
                Ok(items) => println!(
                    "{}",
                    serde_json::to_string_pretty(&items)
                        .expect("serialize promotion blocked items")
                ),
                Err(err) => {
                    eprintln!("promotion_blocked_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::CandidateLifecycle(run_id)) => {
            match candidate_lifecycle(".", "memory", &run_id) {
                Ok(item) => println!(
                    "{}",
                    serde_json::to_string_pretty(&item).expect("serialize candidate lifecycle")
                ),
                Err(err) => {
                    eprintln!("candidate_lifecycle_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::SuperviseTask {
            task_path,
            max_rounds,
        }) => {
            match supervise_task(".", "memory", &task_path, max_rounds) {
                Ok(run) => println!(
                    "{}",
                    serde_json::to_string_pretty(&run).expect("serialize supervised run")
                ),
                Err(err) => {
                    eprintln!("supervise_task_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::LastSupervisedRun) => {
            match print_last_supervised_run("memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("last_supervised_run_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::SupervisedRunReport(supervised_run_id)) => {
            match print_supervised_run_report("memory", &supervised_run_id) {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("supervised_run_report_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ListSupervisedRuns) => {
            match list_supervised_runs("memory") {
                Ok(runs) => println!("{}", runs.join("\n")),
                Err(err) => {
                    eprintln!("list_supervised_runs_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::EvaStatus) => {
            match print_eva_status(".", "memory") {
                Ok(status) => println!("{status}"),
                Err(err) => {
                    eprintln!("eva_status_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ProofReport) => {
            match print_proof_report(".", "memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("proof_report_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ProofJson) => {
            match print_proof_json(".", "memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("proof_json_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::Demo) => {
            match run_demo(".", "memory") {
                Ok(output) => println!("{output}"),
                Err(err) => {
                    eprintln!("demo_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ApproveCandidate { run_id, reason }) => {
            match approve_candidate(".", "memory", &run_id, &reason) {
                Ok(record) => println!(
                    "{}",
                    serde_json::to_string_pretty(&record).expect("serialize approval")
                ),
                Err(err) => {
                    eprintln!("approve_candidate_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::RejectCandidate { run_id, reason }) => {
            match reject_candidate(".", "memory", &run_id, &reason) {
                Ok(record) => println!(
                    "{}",
                    serde_json::to_string_pretty(&record).expect("serialize rejection")
                ),
                Err(err) => {
                    eprintln!("reject_candidate_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::DeferCandidate { run_id, reason }) => {
            match defer_candidate(".", "memory", &run_id, &reason) {
                Ok(record) => println!(
                    "{}",
                    serde_json::to_string_pretty(&record).expect("serialize defer")
                ),
                Err(err) => {
                    eprintln!("defer_candidate_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ApprovalStatus(run_id)) => {
            match approval_status(".", "memory", &run_id) {
                Ok(status) => println!(
                    "{}",
                    serde_json::to_string_pretty(&status).expect("serialize approval status")
                ),
                Err(err) => {
                    eprintln!("approval_status_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ApprovalLog) => {
            match approval_log("memory") {
                Ok(log) => println!(
                    "{}",
                    serde_json::to_string_pretty(&log).expect("serialize approval log")
                ),
                Err(err) => {
                    eprintln!("approval_log_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::GovernanceStatus) => {
            match governance_status(".", "memory") {
                Ok(status) => println!(
                    "{}",
                    serde_json::to_string_pretty(&status).expect("serialize governance status")
                ),
                Err(err) => {
                    eprintln!("governance_status_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::PromotionReadyApproved) => {
            match promotion_ready_approved(".", "memory") {
                Ok(items) => println!(
                    "{}",
                    serde_json::to_string_pretty(&items)
                        .expect("serialize promotion ready approved")
                ),
                Err(err) => {
                    eprintln!("promotion_ready_approved_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::PromoteApproved(run_id)) => {
            match promote_approved_candidate(".", "memory", &run_id) {
                Ok(status) => println!("{status}"),
                Err(err) => {
                    eprintln!("promote_approved_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ReleaseProposal) => {
            match print_release_proposal(".", "memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("release_proposal_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ReleaseProposalJson) => {
            match print_release_proposal_json(".", "memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("release_proposal_json_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ProofSnapshot) => {
            match print_proof_snapshot(".", "memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("proof_snapshot_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ProofSnapshotJson) => {
            match print_proof_snapshot_json(".", "memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("proof_snapshot_json_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ReleasePreflight(run_id)) => {
            match print_release_preflight_json(".", "memory", &run_id) {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("release_preflight_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ReleaseBundle(run_id)) => {
            match print_release_bundle_json(".", "memory", &run_id) {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("release_bundle_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ReleaseApprove(run_id)) => {
            match print_release_approve(".", "memory", &run_id) {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("release_approve_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ReleaseManifest(release_id)) => {
            match print_release_manifest("memory", &release_id) {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("release_manifest_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ReleaseChangelog(release_id)) => {
            match print_release_changelog("memory", &release_id) {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("release_changelog_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::RollbackManifest(release_id)) => {
            match print_rollback_manifest("memory", &release_id) {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("rollback_manifest_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ListReleases) => {
            match list_releases("memory") {
                Ok(ids) => println!("{}", ids.join("\n")),
                Err(err) => {
                    eprintln!("list_releases_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::LastRelease) => {
            match print_last_release("memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("last_release_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ReleaseStatus) => {
            match print_release_status("memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("release_status_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ReleaseHealth) => {
            match print_release_health(".", "memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("release_health_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ReleaseHealthJson) => {
            match print_release_health_json(".", "memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("release_health_json_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ArtifactAudit) => {
            match print_artifact_audit(".") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("artifact_audit_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ArtifactAuditJson) => {
            match print_artifact_audit_json(".") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("artifact_audit_json_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::DeterminismAudit) => {
            match print_determinism_audit(".", "memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("determinism_audit_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::DeterminismAuditJson) => {
            match print_determinism_audit_json(".", "memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("determinism_audit_json_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::PreflightGate) => {
            match print_preflight_gate(".", "memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("preflight_gate_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::PreflightGateJson) => {
            match print_preflight_gate_json(".", "memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("preflight_gate_json_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ReleaseLedger) => {
            match print_release_ledger("memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("release_ledger_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ReleaseLedgerJson) => {
            match print_release_ledger_json("memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("release_ledger_json_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::RecordReleaseAttempt(release_id)) => {
            match print_record_release_attempt(".", "memory", &release_id) {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("record_release_attempt_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::FuturePhases) => {
            println!("{}", print_future_phases());
            return;
        }
        Ok(RuntimeCliCommand::FuturePhasesJson) => {
            match print_future_phases_json() {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("future_phases_json_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::OperatorRunbook) => {
            match print_operator_runbook(".", "memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("operator_runbook_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::OpsStatus) => {
            match print_ops_status(".", "memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("ops_status_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::OpsJson) => {
            match print_ops_json(".", "memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("ops_json_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::PrPackage) => {
            match build_pr_package(".", "memory") {
                Ok(report) => println!(
                    "{}",
                    serde_json::to_string_pretty(&report).expect("serialize pr package")
                ),
                Err(err) => {
                    eprintln!("pr_package_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::LastPrPackage) => {
            match print_last_pr_package("memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("last_pr_package_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ListPrPackages) => {
            match list_pr_packages("memory") {
                Ok(items) => println!("{}", items.join("\n")),
                Err(err) => {
                    eprintln!("list_pr_packages_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ExternalPatchPackage(repo_path)) => {
            match build_external_patch_package("memory", &repo_path) {
                Ok(report) => println!(
                    "{}",
                    serde_json::to_string_pretty(&report)
                        .expect("serialize external patch package")
                ),
                Err(err) => {
                    eprintln!("external_patch_package_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::LastExternalPatchPackage) => {
            match print_last_external_patch_package("memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("last_external_patch_package_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ListExternalPatchPackages) => {
            match list_external_patch_packages("memory") {
                Ok(items) => println!("{}", items.join("\n")),
                Err(err) => {
                    eprintln!("list_external_patch_packages_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::SelfReviewPackage) => {
            match build_self_review_package(".", "memory") {
                Ok(report) => println!(
                    "{}",
                    serde_json::to_string_pretty(&report).expect("serialize self review package")
                ),
                Err(err) => {
                    eprintln!("self_review_package_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::LastSelfReviewPackage) => {
            match print_last_self_review_package("memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("last_self_review_package_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ListSelfReviewPackages) => {
            match list_self_review_packages("memory") {
                Ok(items) => println!("{}", items.join("\n")),
                Err(err) => {
                    eprintln!("list_self_review_packages_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::OperatorConsole) => {
            match print_operator_console(".", "memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("operator_console_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::CapabilityPolicy) => {
            match print_capability_policy() {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("capability_policy_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::TrustDecision) => {
            match print_trust_decision(".", "memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("trust_decision_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::WorkspaceSnapshot) => {
            match build_workspace_snapshot(".", "memory") {
                Ok(report) => println!(
                    "{}",
                    serde_json::to_string_pretty(&report).expect("serialize workspace snapshot")
                ),
                Err(err) => {
                    eprintln!("workspace_snapshot_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::LastWorkspaceSnapshot) => {
            match print_last_workspace_snapshot("memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("last_workspace_snapshot_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ListWorkspaceSnapshots) => {
            match list_workspace_snapshots("memory") {
                Ok(items) => println!("{}", items.join("\n")),
                Err(err) => {
                    eprintln!("list_workspace_snapshots_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::EvidenceBundle) => {
            match build_evidence_bundle(".", "memory") {
                Ok(report) => println!(
                    "{}",
                    serde_json::to_string_pretty(&report).expect("serialize evidence bundle")
                ),
                Err(err) => {
                    eprintln!("evidence_bundle_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::LastEvidenceBundle) => {
            match print_last_evidence_bundle("memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("last_evidence_bundle_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ListEvidenceBundles) => {
            match list_evidence_bundles("memory") {
                Ok(items) => println!("{}", items.join("\n")),
                Err(err) => {
                    eprintln!("list_evidence_bundles_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::RecoveryManifest) => {
            match build_recovery_manifest(".", "memory") {
                Ok(report) => println!(
                    "{}",
                    serde_json::to_string_pretty(&report).expect("serialize recovery manifest")
                ),
                Err(err) => {
                    eprintln!("recovery_manifest_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::LastRecoveryManifest) => {
            match print_last_recovery_manifest("memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("last_recovery_manifest_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::ListRecoveryManifests) => {
            match list_recovery_manifests("memory") {
                Ok(items) => println!("{}", items.join("\n")),
                Err(err) => {
                    eprintln!("list_recovery_manifests_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::PreflightGateV3) => {
            match print_preflight_gate_v3(".", "memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("preflight_gate_v3_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::TrustProofReport) => {
            match print_trust_proof_report(".", "memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("trust_proof_report_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::RuntimeCandidate) => {
            match print_runtime_candidate(".", "memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("runtime_candidate_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::RuntimeValidation) => {
            match print_runtime_validation(".", "memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("runtime_validation_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::RuntimeService) => {
            match print_runtime_service("memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("runtime_service_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::RuntimeCliContract) => {
            match print_runtime_cli_contract("memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("runtime_cli_contract_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::FinalRcReport) => {
            match print_final_rc_report(".", "memory") {
                Ok(report) => println!("{report}"),
                Err(err) => {
                    eprintln!("final_rc_report_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::DistillPatterns) => {
            match distill_patterns("memory") {
                Ok(summary) => println!(
                    "{}",
                    serde_json::to_string_pretty(&summary).expect("serialize pattern summary")
                ),
                Err(err) => {
                    eprintln!("distill_patterns_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::RecombinePatterns) => {
            match render_recombined_hypotheses("memory") {
                Ok(output) => println!("{output}"),
                Err(err) => {
                    eprintln!("recombine_patterns_error: {err}");
                    std::process::exit(1);
                }
            }
            return;
        }
        Ok(RuntimeCliCommand::EvolveRecombined) => {
            if let Err(err) = run_recombined_evolution_cycle(".", "memory") {
                eprintln!("evolve_recombined_error: {err}");
                std::process::exit(1);
            }
            println!("evolve_recombined_status: ok");
            return;
        }
        Ok(RuntimeCliCommand::Replay(run_id)) => {
            if let Err(err) = replay_candidate(".", "memory", &run_id) {
                eprintln!("replay_error: {err}");
                std::process::exit(1);
            }

            match read_replay_cli_status("memory", &run_id) {
                Ok(status) => println!("replay_status: {status}"),
                Err(err) => {
                    eprintln!("replay_status_error: {err}");
                    std::process::exit(1);
                }
            }

            return;
        }
        Ok(RuntimeCliCommand::Promote(run_id)) => {
            if let Err(err) = promote_candidate(".", "memory", &run_id) {
                eprintln!("promotion_error: {err}");
                std::process::exit(1);
            }
            println!("promotion_status: ok");
            return;
        }
        Ok(RuntimeCliCommand::IngestRepo(path)) => {
            if let Err(err) = ingest_repo_patterns(&path, "memory") {
                eprintln!("ingest_repo_error: {err}");
                std::process::exit(1);
            }
            println!("ingest_repo_status: ok");
            return;
        }
        Ok(RuntimeCliCommand::Serve(config)) => {
            if let Err(err) = serve_runtime_daemon(config) {
                eprintln!("runtime_daemon_error: {err}");
                std::process::exit(1);
            }
            return;
        }
        Err(err) => {
            eprintln!("runtime_cli_error: {err}");
            eprintln!("run `cargo run` for available commands");
            std::process::exit(1);
        }
    }

    let input = load_input("input.json").unwrap_or_else(|_| CycleInput {
        goal: "получить фазовый отчёт EVA по локальному runtime циклу".to_string(),
        external_state: "локальный demo режим без внешних сервисов".to_string(),
    });

    let mut runner = RuntimeCycleRunner::new();
    match runner.run_cycle_report(input) {
        Ok(report) => {
            let output = build_project_phase_runtime_output(&report);
            println!(
                "{}",
                serde_json::to_string_pretty(&output).expect("serialize runtime phase output")
            );
        }
        Err(err) => {
            eprintln!("runtime_cycle_error: {err}");
            std::process::exit(1);
        }
    }
}

fn resolve_corpus_alias(memory_root: &str, corpus_id: &str) -> Result<String, String> {
    if corpus_id == "latest" {
        latest_corpus_id(memory_root)
    } else {
        Ok(corpus_id.to_string())
    }
}

fn render_corpora_listing(memory_root: &str, corpora: &[String]) -> String {
    let mut lines = Vec::new();
    for corpus_id in corpora {
        if let Ok(summary) = load_corpus_summary(memory_root, corpus_id) {
            lines.push(format!(
                "{} root_path={} scanned_files={} detected_strategy_count={}",
                summary.corpus_id,
                summary.root_path,
                summary.scanned_files,
                summary.suggested_strategies.len()
            ));
        } else {
            lines.push(corpus_id.clone());
        }
    }
    lines.join("\n")
}

#[derive(Debug, Deserialize)]
struct ReplayCliStatus {
    replay_status: eva_runtime_with_task_validator::EvolutionStatus,
    matches_stored_summary: bool,
    cargo_check_ok: bool,
    cargo_test_ok: bool,
    cargo_run_ok: bool,
}

fn read_replay_cli_status(memory_root: &str, run_id: &str) -> Result<String, String> {
    let path = Path::new(memory_root)
        .join("replays")
        .join(format!("{run_id}.json"));

    let contents = fs::read_to_string(&path)
        .map_err(|err| format!("failed to read replay result {}: {err}", path.display()))?;

    let status: ReplayCliStatus = serde_json::from_str(&contents)
        .map_err(|err| format!("failed to parse replay result {}: {err}", path.display()))?;

    if status.matches_stored_summary
        && status.replay_status != eva_runtime_with_task_validator::EvolutionStatus::Failed
        && status.cargo_check_ok
        && status.cargo_test_ok
        && status.cargo_run_ok
    {
        Ok("passed".to_string())
    } else if status.replay_status == eva_runtime_with_task_validator::EvolutionStatus::Failed
        || !status.matches_stored_summary
        || !status.cargo_check_ok
        || !status.cargo_test_ok
        || !status.cargo_run_ok
    {
        Ok("failed".to_string())
    } else {
        Ok("unknown".to_string())
    }
}

#[derive(Debug, Deserialize)]
struct InputFile {
    goal: String,
    context: String,
}

fn load_input(path: impl AsRef<Path>) -> Result<CycleInput, String> {
    let path = path.as_ref();
    let contents = fs::read_to_string(path).map_err(|err| err.to_string())?;
    let input: InputFile = serde_json::from_str(&contents).map_err(|err| err.to_string())?;
    Ok(CycleInput {
        goal: input.goal,
        external_state: input.context,
    })
}
