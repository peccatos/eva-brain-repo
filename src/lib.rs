pub mod agent;
pub mod benchmark_case_loader;
pub mod benchmark_contract;
pub mod benchmark_metrics;
pub mod benchmark_report;
pub mod benchmark_runner;
pub mod contracts;
pub mod evolution;
pub mod github_tool_contract;
pub mod github_tool_executor;
pub mod graph;
pub mod llm;
pub mod local_model;
pub mod project_phase_report;
pub mod promotion;
pub mod repo_patch_report;
pub mod runtime;
pub mod runtime_cycle;
pub mod runtime_daemon;
pub mod sandbox;
pub mod tool_contract;
pub mod tool_executor;
pub mod tui;

pub use agent::{
    add_specimen, apply_proposal, approve_proposal, build_agent_report, build_pr_summary_for_task,
    build_production_agent_readiness, build_production_agent_v2_readiness, build_repo_map,
    build_task_outcome, create_task, dry_run_apply, inspect_workspace, list_specimens,
    list_task_outcomes, list_tasks, plan_task, plan_task_with_provider, print_agent_readiness,
    print_agent_report, print_agent_v2_readiness, print_apply_dry_run, print_apply_proposal,
    print_approve_proposal, print_create_task, print_fix, print_plan_task,
    print_pr_summary_for_task, print_proposal_show, print_propose_task, print_repo_map,
    print_show_task, print_specimen_add, print_specimen_list, print_task_outcome,
    print_task_outcomes, print_tasks, print_validation_run, print_workspace_inspection,
    proposal_from_llm_response, propose_task, propose_task_with_provider,
    refresh_all_task_outcomes, run_fix, run_validation, show_task, validate_patch_path,
};
pub use benchmark_case_loader::BenchmarkCaseLoader;
pub use benchmark_contract::{
    BenchmarkCaseManifest, BenchmarkFailureType, BenchmarkSourceType, RepositoryDiscoveryCase,
    RepositoryDiscoveryManifest, RustBugfixCase,
};
pub use benchmark_metrics::{BenchmarkAggregateMetrics, BenchmarkCaseMetrics};
pub use benchmark_report::{BenchmarkBatchReport, DEFAULT_BATCH_REPORT_PATH};
pub use benchmark_runner::BenchmarkRunner;
pub use contracts::{
    AgentApproval, AgentPlan, AgentReport, AgentTask, AgentTaskStatus, AgentValidationStatus,
    ApplyResult, ApplyStatus, ApprovalStatus, ArtifactAuditReport, CandidateQueueSummary,
    CandidateState, CapabilityPolicy, CommandResult, CorpusIngestContract, DeniedMutationKind,
    DeterminismAuditReport, EvidenceBundle, EvolutionLogEntry, EvolutionReport, EvolutionStatus,
    ExternalPatchPackage, FinalRcReport, FitnessDecision, FixDetectedProblem, FixEvidencePaths,
    FixMode, FixOnly, FixProblemKind, FixReport, FixRequest, FixRiskCap, FixStatus,
    FuturePhaseEntry, FuturePhaseRegistry, GovernanceStatus, GovernanceTrustGate, LlmPurpose,
    LlmRequest, LlmResponse, LlmStatus, MutationContract, MutationKind, MutationObjective,
    MutationPlan, OperationsReport, OperatorApprovalRecord, OperatorConsoleReport, PatchOp,
    PatchOperationKind, PatchProposal, PlanStep, PrPackage, PrSummary, PreflightGateReport,
    PreflightGateV3Report, ProductionAgentReadiness, ProductionAgentV2Readiness, PromotionQueue,
    PromotionQueueItem, ProofReport, ProofSnapshot, ProposalStatus, RecombinedHypothesis,
    RecoveryManifest, ReleaseBundle, ReleaseCandidateApprovalReport, ReleaseCandidateState,
    ReleaseHealthReport, ReleaseLedgerRecord, ReleaseManifest, ReleasePreflightReport,
    ReleaseProposal, ReleaseProposalItem, RepoMap, RepoModule, RollbackManifest,
    RuntimeCandidateManifest, RuntimeCliCommandContract, RuntimeCliContractReport,
    RuntimeServiceMetadata, RuntimeValidation, SandboxResult, SanitizedContext,
    SelfImprovementProposal, SelfReviewPackage, SpecimenMetadata, SupervisedRun, TaskAdjustment,
    TaskContract, TaskOutcome, TaskOutcomeAnalysis, TaskStrategyMemory, TaskStrategyPattern,
    TrustDecision, TrustProofReport, TuiAgentState, TuiCandidateRow, TuiDashboardState,
    TuiMetricsState, TuiReleaseState, TuiRunRow, TuiState, ValidationCommandResult, ValidationRun,
    ValidationStatus, WorkFitnessScore, WorkspaceInspection, WorkspaceSnapshot,
};
pub use evolution::{
    adjust_task_from_campaign, apply_mutation, approval_log, approval_status, approve_candidate,
    approve_release_candidate, autonomy_status, build_agent_patterns, build_artifact_audit,
    build_capability_policy, build_determinism_audit, build_evidence_bundle,
    build_evolution_core_readiness, build_external_patch_package, build_final_rc_report,
    build_fitness, build_future_phase_registry, build_operations_report,
    build_operator_console_report, build_pr_package, build_preflight_gate, build_preflight_gate_v3,
    build_proof_report, build_proof_snapshot, build_recovery_manifest, build_release_bundle,
    build_release_candidate_state, build_release_health, build_release_preflight,
    build_release_proposal, build_runtime_candidate_manifest, build_runtime_cli_contract,
    build_runtime_service_metadata, build_runtime_validation, build_self_review_package,
    build_task_outcome_analysis, build_task_strategy_memory, build_trust_decision,
    build_trust_proof_report, build_workspace_snapshot, candidate_lifecycle, classify_goal,
    classify_mutation_kind, classify_mutation_kind_label, classify_run_outcome,
    compute_quality_for_hypothesis, compute_quality_for_run, count_sandbox_leaks,
    default_corpus_contract, defer_candidate, distill_patterns, ensure_portfolio,
    evaluate_runtime_validation, fix_generated_test_names, generate_from_plan,
    generate_from_recombined_hypothesis, generate_safe_mutation, governance_status,
    governance_trust_gate, ingest_corpus, latest_corpus_id, latest_proof_snapshot_id,
    latest_record_for_run, latest_release_id, latest_supervised_run_id, learning_summary,
    list_adjusted_tasks, list_bounded_runs, list_corpora, list_evidence_bundles,
    list_external_patch_packages, list_pr_packages, list_recovery_manifests, list_releases,
    list_self_review_packages, list_suggested_tasks, list_supervised_runs,
    list_workspace_snapshots, load_corpus_patterns, load_corpus_summary, load_metrics,
    load_metrics_snapshot, load_or_refresh_evolution_policy, load_or_refresh_promotion_queue,
    load_policy_feedback, load_portfolio, load_promotion_queue, load_recombined_hypotheses,
    load_report_json, load_strategy_portfolio, mutation_class_label,
    normalized_generated_test_name, preview_campaign_recombination, print_artifact_audit,
    print_artifact_audit_json, print_benchmark, print_bounded_run_report, print_campaign,
    print_campaign_report, print_capability_policy, print_determinism_audit,
    print_determinism_audit_json, print_eva_status, print_evolution_policy, print_final_rc_report,
    print_fitness, print_future_phases, print_future_phases_json, print_hygiene_plan,
    print_hygiene_report, print_last_bounded_run, print_last_campaign_report,
    print_last_evidence_bundle, print_last_external_patch_package, print_last_pr_package,
    print_last_recovery_manifest, print_last_release, print_last_report,
    print_last_self_review_package, print_last_supervised_run, print_last_task_adjustment,
    print_last_workspace_snapshot, print_operator_console, print_operator_runbook, print_ops_json,
    print_ops_status, print_outcome_analyze, print_patterns, print_portfolio, print_preflight_gate,
    print_preflight_gate_json, print_preflight_gate_v3, print_promotion_queue, print_proof_json,
    print_proof_report, print_proof_snapshot, print_proof_snapshot_json, print_quality_report,
    print_record_release_attempt, print_release_approve, print_release_bundle_json,
    print_release_changelog, print_release_health, print_release_health_json, print_release_ledger,
    print_release_ledger_json, print_release_manifest, print_release_preflight_json,
    print_release_proposal, print_release_proposal_json, print_release_status, print_report,
    print_rollback_manifest, print_runtime_candidate, print_runtime_cli_contract,
    print_runtime_service, print_runtime_validation, print_self_improve_propose,
    print_strategy_memory, print_strategy_portfolio, print_strategy_select,
    print_supervised_run_report, print_trust_decision, print_trust_proof_report,
    promote_approved_candidate, promotion_blocked_items, promotion_ready_approved,
    promotion_ready_items, propose_self_improvement, rank_plans, record_evolution,
    record_promotion_event, record_release_attempt, refresh_evolution_policy, refresh_metrics,
    refresh_portfolio, refresh_promotion_queue, refresh_report, refresh_strategy_portfolio,
    reject_candidate, release_count, release_ledger_count, release_proposal_count,
    render_recombined_hypotheses, run_benchmark, run_bounded_evolution, run_demo,
    run_evolution_hygiene, run_planned_cycles, run_stored_campaign, run_task_from_path,
    score_cycle, score_task_outcome, select_strategy, select_task_compatible_from_hypotheses,
    select_task_compatible_hypothesis, suggest_strategy_tasks, supervise_task,
    top_recombined_hypothesis, update_metrics_after_log, update_policy_feedback,
    update_portfolio_after_log, update_portfolio_after_replay, validate_corpus_contract,
    validate_mutation, validate_task_contract, write_report, AutonomyStatus, BoundedRunSummary,
    CampaignBlockerCount, CampaignRecombinationDiagnostics, CampaignRecombinationPreview,
    CorpusPatterns, CorpusSummary, DistilledPatternSummary, EvolutionBenchmark, EvolutionCampaign,
    EvolutionHypothesis, EvolutionMetrics, EvolutionPolicy, EvolutionRunOutcome, EvolutionScore,
    HygieneReport, LearningContext, MutationClass, MutationPortfolio, MutationPortfolioEntry,
    PolicyFeedback, QualityMetricsV2, StrategyPortfolio, StrategyPortfolioEntry,
};
pub use github_tool_contract::{DiscoveryConfig, GithubRepositorySummary, GithubSearchFixture};
pub use github_tool_executor::GithubToolExecutor;
pub use graph::{
    analyzer::propose_mutation_plans, analyzer::propose_mutation_plans_for_task,
    analyzer::render_plans, analyzer::render_task_plans, ast_extract::extract_rust_ast,
    ingest_repo_patterns, update_graph_for_evolution, EvolutionGraph,
};
pub use local_model::{
    models_url_from_chat_endpoint, parse_chat_response, parse_models_response, ModelChatMessage,
    ModelChatOptions, ModelChatOutput, ModelHealth, OpenAiModelClient, OpenAiModelConfig,
    BUILTIN_MODEL_ENDPOINT, BUILTIN_MODEL_NAME, DEFAULT_MODEL_ID, DEFAULT_MODEL_NAME,
    DEFAULT_MODEL_URL,
};
pub use project_phase_report::{
    build_runtime_output as build_project_phase_runtime_output, ProjectPhaseReport,
    ProjectPhaseRuntimeOutput, ProjectPhaseStatus,
};
pub use promotion::{
    candidate_diff, check_promotion_gate, list_candidates, promote_candidate, replay_candidate,
    review_candidate, review_report_markdown, CandidateReview, CandidateReviewReport,
    PromotionDecision,
};
pub use repo_patch_report::{
    run_repo_patch_report, should_run_repo_patch_mode, RepoChangeType, RepoChangedFile,
    RepoPatchCliConfig, RepoPatchExecution, RepoPatchMachineSummary, RepoPatchStatus,
};
pub use runtime::{
    run_evolution_cycle, run_evolution_cycle_with_memory, run_planned_evolution_cycle,
    run_planned_evolution_cycle_for_task, run_recombined_evolution_cycle,
    run_recombined_evolution_cycle_for_hypothesis,
};
pub use runtime_cycle::{CycleInput, RuntimeAudit, RuntimeCycleReport, RuntimeCycleRunner};
pub use runtime_daemon::{
    handle_http_request, serve as serve_runtime_daemon, DaemonHealthResponse, HttpResponse,
    ManagedServerConfig, ModelBackendHealth, ModelChatHttpRequest, ModelRegistryResponse,
    RuntimeCliCommand, RuntimeCycleHttpRequest, RuntimeCycleHttpResponse, RuntimeDaemonConfig,
    RuntimeModelAdvisory, DEFAULT_LISTEN_ADDR, DEFAULT_RUNTIME_CONFIG_PATH, RUNTIME_CLI_HELP,
};
pub use sandbox::{
    copy_project, create_sandbox_path, destroy_sandbox, run_cargo_check, run_cargo_run,
    run_cargo_test,
};
pub use tool_contract::{CommandOutput, ToolRequest, ToolResponse};
pub use tool_executor::ToolExecutor;
pub use tui::{
    format_unknown, load_tui_state, load_tui_state_from_project_root, render_tui_snapshot,
    run_interactive_tui, run_tui,
};
