pub mod artifact_audit;
pub mod autonomy;
pub mod benchmark;
pub mod bounded_loop;
pub mod campaign;
pub mod campaign_recombination;
pub mod capability_policy;
pub mod changelog;
pub mod ci_pr;
pub mod classification;
pub mod corpus;
pub mod corpus_validator;
pub mod dedup;
pub mod determinism_audit;
pub mod evidence_bundle;
pub mod evolution_core_readiness;
pub mod evolution_policy;
pub mod external_patch;
pub mod final_rc_report;
pub mod future_phase;
pub mod generator;
pub mod governance;
pub mod hygiene;
pub mod hypothesis;
pub mod learning_context;
pub mod memory;
pub mod metrics;
pub mod mutation_portfolio;
pub mod mutator;
pub mod operations;
pub mod operator_approval;
pub mod operator_console;
pub mod operator_runbook;
pub mod patterns;
pub mod policy_feedback;
pub mod preflight_gate;
pub mod preflight_gate_v3;
pub mod promotion_queue;
pub mod proof;
pub mod proof_snapshot;
pub mod quality;
pub mod recombination;
pub mod recovery_manifest;
pub mod regression_memory;
pub mod release_bundle;
pub mod release_candidate;
pub mod release_health;
pub mod release_ledger;
pub mod release_preflight;
pub mod release_proposal;
pub mod report_ru;
pub mod rollback;
pub mod runtime_candidate;
pub mod runtime_cli_contract;
pub mod runtime_service;
pub mod runtime_validation;
pub mod scorer;
pub mod self_review;
pub mod strategy_portfolio;
pub mod strategy_task_suggester;
pub mod success_memory;
pub mod supervisor;
pub mod task_validator;
pub mod task_yield;
pub mod templates;
pub mod trust_decision;
pub mod trust_proof_report;
pub mod validator;
pub mod workspace_snapshot;

pub use artifact_audit::{build_artifact_audit, print_artifact_audit, print_artifact_audit_json};
pub use autonomy::{autonomy_status, AutonomyStatus};
pub use benchmark::{
    count_sandbox_leaks, print_benchmark, run_benchmark, run_planned_cycles, EvolutionBenchmark,
};
pub use bounded_loop::{
    list_bounded_runs, print_bounded_run_report, print_last_bounded_run, run_bounded_evolution,
    BoundedRunSummary,
};
pub use campaign::{
    print_campaign, print_campaign_report, print_last_campaign_report, run_stored_campaign,
    run_task_from_path, CampaignBlockerCount, EvolutionCampaign,
};
pub use campaign_recombination::{
    preview_campaign_recombination, select_task_compatible_from_hypotheses,
    select_task_compatible_hypothesis, CampaignRecombinationDiagnostics,
    CampaignRecombinationPreview,
};
pub use capability_policy::{build_capability_policy, print_capability_policy};
pub use ci_pr::{build_pr_package, list_pr_packages, print_last_pr_package};
pub use classification::{
    classify_mutation_kind, classify_mutation_kind_label, mutation_class_label, MutationClass,
};
pub use corpus::{
    default_corpus_contract, ingest_corpus, latest_corpus_id, list_corpora, load_corpus_patterns,
    load_corpus_summary, CorpusPatterns, CorpusSummary,
};
pub use corpus_validator::validate_corpus_contract;
pub use dedup::{
    compute_mutation_digest, load_dedup_entries, record_dedup_entry, should_reject_duplicate_bad,
    DedupEntry,
};
pub use determinism_audit::{
    build_determinism_audit, print_determinism_audit, print_determinism_audit_json,
};
pub use evidence_bundle::{
    build_evidence_bundle, latest_evidence_bundle_id, list_evidence_bundles,
    print_last_evidence_bundle,
};
pub use evolution_core_readiness::build_evolution_core_readiness;
pub use evolution_policy::{
    load_or_refresh_evolution_policy, print_evolution_policy, refresh_evolution_policy,
    EvolutionPolicy,
};
pub use external_patch::{
    build_external_patch_package, list_external_patch_packages, print_last_external_patch_package,
};
pub use final_rc_report::{build_final_rc_report, print_final_rc_report};
pub use future_phase::{
    build_future_phase_registry, print_future_phases, print_future_phases_json,
};
pub use generator::{
    generate_from_plan, generate_from_recombined_hypothesis, generate_safe_mutation,
};
pub use governance::{
    governance_status, governance_trust_gate, promote_approved_candidate, promotion_ready_approved,
};
pub use hygiene::{
    fix_generated_test_names, print_hygiene_plan, print_hygiene_report, run_evolution_hygiene,
    HygieneReport,
};
pub use hypothesis::{rank_plans, EvolutionHypothesis};
pub use learning_context::LearningContext;
pub use memory::{record_evolution, CandidateSummary, ReplayResult};
pub use metrics::{
    classify_run_outcome, learning_summary, load_metrics, load_metrics_snapshot, refresh_metrics,
    update_metrics_after_log, EvolutionMetrics, EvolutionRunOutcome,
};
pub use mutation_portfolio::{
    ensure_portfolio, kind_label as portfolio_kind_label, load_portfolio, print_portfolio,
    refresh_portfolio, update_portfolio_after_log, update_portfolio_after_replay,
    MutationPortfolio, MutationPortfolioEntry,
};
pub use mutator::apply_mutation;
pub use operations::{build_operations_report, print_ops_json, print_ops_status};
pub use operator_approval::{
    approval_log, approval_status, approve_candidate, defer_candidate, latest_decisions,
    latest_record_for_run, record_promotion_event, reject_candidate,
};
pub use operator_console::{build_operator_console_report, print_operator_console};
pub use operator_runbook::print_operator_runbook;
pub use patterns::{distill_patterns, DistilledPatternSummary};
pub use policy_feedback::{load_policy_feedback, update_policy_feedback, PolicyFeedback};
pub use preflight_gate::{build_preflight_gate, print_preflight_gate, print_preflight_gate_json};
pub use preflight_gate_v3::{build_preflight_gate_v3, print_preflight_gate_v3};
pub use promotion_queue::{
    candidate_lifecycle, load_or_refresh_promotion_queue, load_promotion_queue,
    print_promotion_queue, promotion_blocked_items, promotion_ready_items, refresh_promotion_queue,
};
pub use proof::{
    build_proof_report, print_eva_status, print_proof_json, print_proof_report, run_demo,
};
pub use proof_snapshot::{
    build_proof_snapshot, latest_proof_snapshot_id, print_proof_snapshot, print_proof_snapshot_json,
};
pub use quality::{
    compute_quality_for_hypothesis, compute_quality_for_run, print_quality_report, QualityMetricsV2,
};
pub use recombination::{
    load_recombined_hypotheses, render_recombined_hypotheses, top_recombined_hypothesis,
};
pub use recovery_manifest::{
    build_recovery_manifest, latest_recovery_manifest_id, list_recovery_manifests,
    print_last_recovery_manifest,
};
pub use regression_memory::{load_regressions, record_regression, RegressionEntry};
pub use release_bundle::{
    build_release_bundle, latest_release_id, list_releases, print_last_release,
    print_release_bundle_json, print_release_changelog, print_release_manifest,
    print_release_status, print_rollback_manifest, release_count,
};
pub use release_candidate::{
    approve_release_candidate, build_release_candidate_state, print_release_approve,
};
pub use release_health::{build_release_health, print_release_health, print_release_health_json};
pub use release_ledger::{
    latest_release_or_none, load_release_ledger, print_record_release_attempt,
    print_release_ledger, print_release_ledger_json, record_release_attempt, release_ledger_count,
};
pub use release_preflight::{build_release_preflight, print_release_preflight_json};
pub use release_proposal::{
    build_release_proposal, print_release_proposal, print_release_proposal_json,
    release_proposal_count,
};
pub use report_ru::{
    load_report_json, print_last_report, print_report, refresh_report, write_report,
};
pub use rollback::rollback_sandbox;
pub use runtime_candidate::{
    build_runtime_candidate_manifest, print_runtime_candidate, proof_support_flags,
};
pub use runtime_cli_contract::{build_runtime_cli_contract, print_runtime_cli_contract};
pub use runtime_service::{build_runtime_service_metadata, print_runtime_service};
pub use runtime_validation::{
    build_runtime_validation, evaluate_runtime_validation, load_latest_runtime_validation,
    load_or_build_runtime_validation, print_runtime_validation,
};
pub use scorer::{score_cycle, EvolutionScore};
pub use self_review::{
    build_self_review_package, list_self_review_packages, print_last_self_review_package,
};
pub use strategy_portfolio::{
    ensure_strategy_portfolio, infer_strategy, load_strategy_portfolio, print_strategy_portfolio,
    refresh_strategy_portfolio, StrategyPortfolio, StrategyPortfolioEntry,
};
pub use strategy_task_suggester::{list_suggested_tasks, suggest_strategy_tasks};
pub use success_memory::{load_success_patterns, record_success_pattern, SuccessPatternEntry};
pub use supervisor::{
    latest_supervised_run_id, list_supervised_runs, print_last_supervised_run,
    print_supervised_run_report, supervise_task,
};
pub use task_validator::{
    load_stored_task_contract, load_task_contract, matches_target_patterns, store_task_contract,
    validate_task_contract,
};
pub use task_yield::{adjust_task_from_campaign, list_adjusted_tasks, print_last_task_adjustment};
pub use templates::normalized_generated_test_name;
pub use trust_decision::{build_trust_decision, print_trust_decision};
pub use trust_proof_report::{build_trust_proof_report, print_trust_proof_report};
pub use validator::validate_mutation;
pub use workspace_snapshot::{
    build_workspace_snapshot, latest_workspace_snapshot_id, list_workspace_snapshots,
    print_last_workspace_snapshot,
};
