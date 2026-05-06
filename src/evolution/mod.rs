pub mod autonomy;
pub mod benchmark;
pub mod campaign;
pub mod classification;
pub mod corpus;
pub mod corpus_validator;
pub mod dedup;
pub mod evolution_policy;
pub mod generator;
pub mod hygiene;
pub mod hypothesis;
pub mod learning_context;
pub mod memory;
pub mod metrics;
pub mod mutation_portfolio;
pub mod mutator;
pub mod patterns;
pub mod quality;
pub mod recombination;
pub mod regression_memory;
pub mod report_ru;
pub mod rollback;
pub mod scorer;
pub mod strategy_portfolio;
pub mod strategy_task_suggester;
pub mod success_memory;
pub mod task_validator;
pub mod templates;
pub mod validator;

pub use autonomy::{autonomy_status, AutonomyStatus};
pub use benchmark::{
    count_sandbox_leaks, print_benchmark, run_benchmark, run_planned_cycles, EvolutionBenchmark,
};
pub use campaign::{
    print_campaign, print_last_campaign_report, run_stored_campaign, run_task_from_path,
    EvolutionCampaign,
};
pub use classification::{
    classify_mutation_kind, classify_mutation_kind_label, mutation_class_label, MutationClass,
};
pub use corpus::{
    default_corpus_contract, ingest_corpus, list_corpora, load_corpus_patterns,
    load_corpus_summary, CorpusPatterns, CorpusSummary,
};
pub use corpus_validator::validate_corpus_contract;
pub use dedup::{
    compute_mutation_digest, load_dedup_entries, record_dedup_entry, should_reject_duplicate_bad,
    DedupEntry,
};
pub use evolution_policy::{
    load_or_refresh_evolution_policy, print_evolution_policy, refresh_evolution_policy,
    EvolutionPolicy,
};
pub use generator::{
    generate_from_plan, generate_from_recombined_hypothesis, generate_safe_mutation,
};
pub use hygiene::{
    fix_generated_test_names, print_hygiene_plan, print_hygiene_report, run_evolution_hygiene,
    HygieneReport,
};
pub use hypothesis::{rank_plans, EvolutionHypothesis};
pub use learning_context::LearningContext;
pub use memory::{record_evolution, CandidateSummary, ReplayResult};
pub use metrics::{
    learning_summary, load_metrics, refresh_metrics, update_metrics_after_log, EvolutionMetrics,
};
pub use mutation_portfolio::{
    ensure_portfolio, kind_label as portfolio_kind_label, load_portfolio, print_portfolio,
    refresh_portfolio, update_portfolio_after_log, update_portfolio_after_replay,
    MutationPortfolio, MutationPortfolioEntry,
};
pub use mutator::apply_mutation;
pub use patterns::{distill_patterns, DistilledPatternSummary};
pub use quality::{
    compute_quality_for_hypothesis, compute_quality_for_run, print_quality_report, QualityMetricsV2,
};
pub use recombination::{
    load_recombined_hypotheses, render_recombined_hypotheses, top_recombined_hypothesis,
};
pub use regression_memory::{load_regressions, record_regression, RegressionEntry};
pub use report_ru::{
    load_report_json, print_last_report, print_report, refresh_report, write_report,
};
pub use rollback::rollback_sandbox;
pub use scorer::{score_cycle, EvolutionScore};
pub use strategy_portfolio::{
    ensure_strategy_portfolio, infer_strategy, load_strategy_portfolio, print_strategy_portfolio,
    refresh_strategy_portfolio, StrategyPortfolio, StrategyPortfolioEntry,
};
pub use strategy_task_suggester::{list_suggested_tasks, suggest_strategy_tasks};
pub use success_memory::{load_success_patterns, record_success_pattern, SuccessPatternEntry};
pub use task_validator::{
    load_stored_task_contract, load_task_contract, matches_target_patterns, store_task_contract,
    validate_task_contract,
};
pub use templates::normalized_generated_test_name;
pub use validator::validate_mutation;
