pub mod autonomy;
pub mod benchmark;
pub mod campaign;
pub mod dedup;
pub mod generator;
pub mod hypothesis;
pub mod learning_context;
pub mod memory;
pub mod metrics;
pub mod mutator;
pub mod patterns;
pub mod recombination;
pub mod regression_memory;
pub mod report_ru;
pub mod rollback;
pub mod scorer;
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
pub use dedup::{
    compute_mutation_digest, load_dedup_entries, record_dedup_entry, should_reject_duplicate_bad,
    DedupEntry,
};
pub use generator::{
    generate_from_plan, generate_from_recombined_hypothesis, generate_safe_mutation,
};
pub use hypothesis::{rank_plans, EvolutionHypothesis};
pub use learning_context::LearningContext;
pub use memory::{record_evolution, CandidateSummary, ReplayResult};
pub use metrics::{
    learning_summary, load_metrics, refresh_metrics, update_metrics_after_log, EvolutionMetrics,
};
pub use mutator::apply_mutation;
pub use patterns::{distill_patterns, DistilledPatternSummary};
pub use recombination::{
    load_recombined_hypotheses, render_recombined_hypotheses, top_recombined_hypothesis,
};
pub use regression_memory::{load_regressions, record_regression, RegressionEntry};
pub use report_ru::{
    load_report_json, print_last_report, print_report, refresh_report, write_report,
};
pub use rollback::rollback_sandbox;
pub use scorer::{score_cycle, EvolutionScore};
pub use success_memory::{load_success_patterns, record_success_pattern, SuccessPatternEntry};
pub use task_validator::{
    load_stored_task_contract, load_task_contract, matches_target_patterns, store_task_contract,
    validate_task_contract,
};
pub use validator::validate_mutation;
