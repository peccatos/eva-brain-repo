use crate::contracts::{
    CommandResult, MutationContract, MutationObjective, RecombinedHypothesis, SandboxResult,
    TaskContract,
};
use crate::evolution::{
    dedup, generator, memory, metrics, mutator, recombination, regression_memory, scorer,
    success_memory, validator, write_report, EvolutionScore, LearningContext,
};
use crate::sandbox::{manager, runner, snapshot};

#[derive(Debug, Clone)]
struct PlanContext {
    plan_id: Option<String>,
    hypothesis_id: Option<String>,
    objective: Option<MutationObjective>,
    graph_evidence: Vec<String>,
    regression_penalty: f32,
    success_bonus: f32,
    recombined_source_patterns: Vec<String>,
    recombined_avoided_risks: Vec<String>,
    recombination_reason_ru: Option<String>,
}

pub fn run_evolution_cycle(project_root: &str) -> Result<(), String> {
    run_evolution_cycle_with_memory(project_root, "memory")
}

pub fn run_evolution_cycle_with_memory(
    project_root: &str,
    memory_root: &str,
) -> Result<(), String> {
    run_evolution_cycle_with_mutation(
        project_root,
        memory_root,
        generator::generate_safe_mutation(),
        None,
    )
}

pub fn run_planned_evolution_cycle(project_root: &str, memory_root: &str) -> Result<(), String> {
    run_planned_evolution_cycle_for_task(project_root, memory_root, None)
}

pub fn run_planned_evolution_cycle_for_task(
    project_root: &str,
    memory_root: &str,
    task: Option<&TaskContract>,
) -> Result<(), String> {
    let plans = crate::graph::analyzer::propose_mutation_plans_for_task(memory_root, task)?;
    let learning = LearningContext::load(memory_root)?;
    let hypotheses = crate::evolution::rank_plans(&plans, &learning);
    let Some(hypothesis) = hypotheses.first() else {
        return Err("no graph-guided plans available".to_string());
    };
    let plan = plans
        .iter()
        .find(|plan| plan.id == hypothesis.plan_id)
        .ok_or_else(|| "ranked hypothesis points to missing plan".to_string())?;
    let mutation = generator::generate_from_plan(plan);
    validator::validate_mutation(&mutation)?;
    let mutation = generator::generate_from_plan(plan);
    if let Some(task) = task {
        if mutation.risk > task.max_risk {
            return Err("planned mutation exceeds task max_risk".to_string());
        }
        if mutation.expected_gain < 0.0 {
            return Err("invalid mutation expected_gain".to_string());
        }
    }
    validator::validate_mutation(&mutation)?;
    run_evolution_cycle_with_mutation(
        project_root,
        memory_root,
        mutation,
        Some(PlanContext {
            plan_id: Some(plan.id.clone()),
            hypothesis_id: Some(hypothesis.id.clone()),
            objective: Some(plan.objective),
            graph_evidence: plan.graph_evidence.clone(),
            regression_penalty: hypothesis.regression_penalty,
            success_bonus: hypothesis.success_bonus,
            recombined_source_patterns: Vec::new(),
            recombined_avoided_risks: Vec::new(),
            recombination_reason_ru: None,
        }),
    )
}

pub fn run_recombined_evolution_cycle(project_root: &str, memory_root: &str) -> Result<(), String> {
    let hypothesis = recombination::top_recombined_hypothesis(memory_root)?;
    let mutation = recombination::generate_from_recombined_hypothesis(&hypothesis)?;
    validator::validate_mutation(&mutation)?;
    run_evolution_cycle_with_mutation(
        project_root,
        memory_root,
        mutation,
        Some(recombined_plan_context(&hypothesis)),
    )
}

fn run_evolution_cycle_with_mutation(
    project_root: &str,
    memory_root: &str,
    mutation: MutationContract,
    plan_context: Option<PlanContext>,
) -> Result<(), String> {
    let run_id = memory::new_run_id();
    let mutation_digest = dedup::compute_mutation_digest(&mutation);
    if dedup::should_reject_duplicate_bad(memory_root, &mutation_digest)? {
        let score = duplicate_rejected_score();
        let sandbox = empty_sandbox_result();
        let preliminary = if let Some(context) = &plan_context {
            memory::build_log_entry_with_plan(
                run_id.clone(),
                context.plan_id.clone(),
                context.hypothesis_id.clone(),
                context.objective.map(|objective| format!("{objective:?}")),
                context.graph_evidence.clone(),
                &mutation,
                mutation_digest.clone(),
                &score,
                &sandbox,
                false,
                true,
                true,
                context.regression_penalty,
                context.success_bonus,
            )
        } else {
            memory::build_log_entry_with_plan(
                run_id.clone(),
                None,
                None,
                None,
                Vec::new(),
                &mutation,
                mutation_digest.clone(),
                &score,
                &sandbox,
                false,
                true,
                true,
                0.0,
                0.0,
            )
        };
        let regression = regression_memory::record_regression(memory_root, &preliminary)?;
        let final_entry = with_learning_adjustments(
            with_recombination_context(preliminary, plan_context.as_ref()),
            regression.penalty
                + plan_context
                    .as_ref()
                    .map(|ctx| ctx.regression_penalty)
                    .unwrap_or(0.0),
            plan_context
                .as_ref()
                .map(|ctx| ctx.success_bonus)
                .unwrap_or(0.0),
        );
        memory::append_jsonl(
            std::path::Path::new(memory_root).join("evolution.jsonl"),
            &final_entry,
        )?;
        write_report(memory_root, &final_entry, &mutation)?;
        dedup::record_dedup_entry(
            memory_root,
            &mutation_digest,
            &mutation,
            final_entry.score,
            final_entry.useful_change,
            &run_id,
        )?;
        metrics::update_metrics_after_log(memory_root, &final_entry)?;
        return Err("duplicate bad mutation rejected before sandbox".to_string());
    }

    let sandbox_path = manager::create_sandbox_path();
    snapshot::copy_project(project_root, &sandbox_path)?;

    let result = run_cycle_in_sandbox(&sandbox_path, mutation);
    let cleanup = manager::destroy_sandbox(&sandbox_path);
    if let Ok((mutation, score, sandbox)) = &result {
        let preliminary = if let Some(context) = &plan_context {
            memory::build_log_entry_with_plan(
                run_id,
                context.plan_id.clone(),
                context.hypothesis_id.clone(),
                context.objective.map(|objective| format!("{objective:?}")),
                context.graph_evidence.clone(),
                mutation,
                mutation_digest.clone(),
                score,
                sandbox,
                false,
                cleanup.is_ok(),
                false,
                context.regression_penalty,
                context.success_bonus,
            )
        } else {
            memory::build_log_entry(
                run_id,
                mutation,
                mutation_digest.clone(),
                score,
                sandbox,
                false,
                cleanup.is_ok(),
            )
        };
        dedup::record_dedup_entry(
            memory_root,
            &mutation_digest,
            mutation,
            preliminary.score,
            preliminary.useful_change,
            &preliminary.run_id,
        )?;
        let regression_penalty = if !preliminary.useful_change
            || preliminary.status == crate::contracts::EvolutionStatus::Failed
        {
            regression_memory::record_regression(memory_root, &preliminary)?.penalty
        } else {
            0.0
        };
        let success_bonus = if preliminary.useful_change
            && preliminary.status == crate::contracts::EvolutionStatus::Candidate
        {
            success_memory::record_success_pattern(memory_root, &preliminary)?.bonus
        } else {
            0.0
        };
        let entry = with_learning_adjustments(
            with_recombination_context(preliminary, plan_context.as_ref()),
            regression_penalty
                + plan_context
                    .as_ref()
                    .map(|ctx| ctx.regression_penalty)
                    .unwrap_or(0.0),
            success_bonus
                + plan_context
                    .as_ref()
                    .map(|ctx| ctx.success_bonus)
                    .unwrap_or(0.0),
        );
        memory::append_jsonl(
            std::path::Path::new(memory_root).join("evolution.jsonl"),
            &entry,
        )?;
        memory::maybe_store_candidate(memory_root, &entry, mutation)?;
        write_report(memory_root, &entry, mutation)?;
        crate::graph::update_graph_for_evolution(memory_root, &entry)?;
        metrics::update_metrics_after_log(memory_root, &entry)?;
    }

    match (result, cleanup) {
        (Ok((_, score, _)), Ok(())) if score.accepted => Ok(()),
        (Ok((_, score, sandbox)), Ok(())) => Err(format!(
            "evolution validation failed: check={} test={} run={}",
            sandbox.check.success, score.test_passed, score.run_passed
        )),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(cleanup_error)) => Err(cleanup_error),
        (Err(error), Err(cleanup_error)) => {
            Err(format!("{error}; cleanup failed: {cleanup_error}"))
        }
    }
}

fn run_cycle_in_sandbox(
    sandbox_path: &str,
    mutation: MutationContract,
) -> Result<
    (
        MutationContract,
        crate::evolution::EvolutionScore,
        SandboxResult,
    ),
    String,
> {
    validator::validate_mutation(&mutation)?;
    mutator::apply_mutation(sandbox_path, &mutation)?;

    let check = runner::run_cargo_check(sandbox_path);
    let test = if check.success {
        runner::run_cargo_test(sandbox_path)
    } else {
        failed_command("cargo test skipped because cargo check failed")
    };
    let run = if test.success {
        Some(runner::run_cargo_run(sandbox_path))
    } else {
        None
    };

    let score = scorer::score_cycle(mutation.kind, &check, &test, run.as_ref());
    let sandbox = SandboxResult {
        sandbox_path: sandbox_path.to_string(),
        check,
        test: Some(test),
        run,
    };
    Ok((mutation, score, sandbox))
}

fn failed_command(stderr: &str) -> CommandResult {
    CommandResult {
        success: false,
        stdout: String::new(),
        stderr: stderr.to_string(),
        duration_ms: 0,
    }
}

fn duplicate_rejected_score() -> EvolutionScore {
    EvolutionScore {
        accepted: false,
        score: 0.0,
        useful_change: false,
        non_candidate_reason: Some("duplicate_bad_mutation".to_string()),
        check_passed: false,
        test_passed: false,
        run_passed: false,
        total_duration_ms: 0,
    }
}

fn empty_sandbox_result() -> SandboxResult {
    SandboxResult {
        sandbox_path: String::new(),
        check: failed_command("sandbox skipped due to duplicate rejection"),
        test: None,
        run: None,
    }
}

fn with_learning_adjustments(
    mut entry: crate::contracts::EvolutionLogEntry,
    regression_penalty: f32,
    success_bonus: f32,
) -> crate::contracts::EvolutionLogEntry {
    entry.regression_penalty = regression_penalty;
    entry.success_bonus = success_bonus;
    entry
}

fn with_recombination_context(
    mut entry: crate::contracts::EvolutionLogEntry,
    context: Option<&PlanContext>,
) -> crate::contracts::EvolutionLogEntry {
    if let Some(context) = context {
        entry.recombined_source_patterns = context.recombined_source_patterns.clone();
        entry.recombined_avoided_risks = context.recombined_avoided_risks.clone();
        entry.recombination_reason_ru = context.recombination_reason_ru.clone();
    }
    entry
}

fn recombined_plan_context(hypothesis: &RecombinedHypothesis) -> PlanContext {
    PlanContext {
        plan_id: None,
        hypothesis_id: Some(hypothesis.hypothesis_id.clone()),
        objective: Some(match hypothesis.target_objective.as_str() {
            "ImproveTests" => MutationObjective::ImproveTests,
            "ImproveValidation" => MutationObjective::ImproveValidation,
            "ImproveReplayability" => MutationObjective::ImproveReplayability,
            "ImproveGraphMemory" => MutationObjective::ImproveGraphMemory,
            "ImproveScoring" => MutationObjective::ImproveScoring,
            "ReduceStorage" => MutationObjective::ReduceStorage,
            "ReduceRuntimeCost" => MutationObjective::ReduceRuntimeCost,
            _ => MutationObjective::ImproveReliability,
        }),
        graph_evidence: hypothesis.source_patterns.clone(),
        regression_penalty: 0.0,
        success_bonus: 0.0,
        recombined_source_patterns: hypothesis.source_patterns.clone(),
        recombined_avoided_risks: hypothesis.avoided_risks.clone(),
        recombination_reason_ru: Some(hypothesis.reason_ru.clone()),
    }
}
