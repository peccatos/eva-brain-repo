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

pub use benchmark_case_loader::BenchmarkCaseLoader;
pub use benchmark_contract::{
    BenchmarkCaseManifest, BenchmarkFailureType, BenchmarkSourceType, RepositoryDiscoveryCase,
    RepositoryDiscoveryManifest, RustBugfixCase,
};
pub use benchmark_metrics::{BenchmarkAggregateMetrics, BenchmarkCaseMetrics};
pub use benchmark_report::{BenchmarkBatchReport, DEFAULT_BATCH_REPORT_PATH};
pub use benchmark_runner::BenchmarkRunner;
pub use contracts::{
    CommandResult, DeniedMutationKind, EvolutionLogEntry, EvolutionReport, MutationContract,
    MutationKind, MutationObjective, MutationPlan, RecombinedHypothesis, SandboxResult,
    TaskContract, ValidationStatus,
};
pub use evolution::{
    apply_mutation, autonomy_status, count_sandbox_leaks, distill_patterns, generate_from_plan,
    generate_from_recombined_hypothesis, generate_safe_mutation, learning_summary, load_metrics,
    load_portfolio, load_recombined_hypotheses, load_report_json, normalized_generated_test_name,
    print_benchmark, print_campaign, print_last_campaign_report, print_last_report,
    print_portfolio, print_report, rank_plans, record_evolution, refresh_metrics, refresh_report,
    render_recombined_hypotheses, run_benchmark, run_planned_cycles, run_stored_campaign,
    run_task_from_path, score_cycle, top_recombined_hypothesis, update_metrics_after_log,
    update_portfolio_after_log, update_portfolio_after_replay, validate_mutation,
    validate_task_contract, write_report, AutonomyStatus, DistilledPatternSummary,
    EvolutionBenchmark, EvolutionCampaign, EvolutionHypothesis, EvolutionMetrics, EvolutionScore,
    LearningContext, MutationPortfolio, MutationPortfolioEntry,
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
