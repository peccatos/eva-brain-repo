use std::path::Path;

use crate::contracts::{MutationKind, MutationObjective, MutationPlan, TaskContract};
use crate::evolution::generator::default_kind_for_objective;
use crate::evolution::{matches_target_patterns, LearningContext};
use crate::graph::load_graph;

pub fn propose_mutation_plans(memory_root: &str) -> Result<Vec<MutationPlan>, String> {
    propose_mutation_plans_for_task(memory_root, None)
}

pub fn propose_mutation_plans_for_task(
    memory_root: &str,
    task: Option<&TaskContract>,
) -> Result<Vec<MutationPlan>, String> {
    let graph = load_graph(&Path::new(memory_root).join("graph.json"))?;
    let learning = LearningContext::load(memory_root).unwrap_or_default();
    let mut safe_files = graph
        .nodes
        .iter()
        .filter(|node| node.kind == "File" || node.kind == "TargetFile")
        .filter_map(|node| node.id.strip_prefix("file:").map(str::to_string))
        .filter(|file| is_safe_target(file))
        .collect::<Vec<_>>();
    safe_files.sort();
    safe_files.dedup();
    safe_files.sort_by(|left, right| {
        let left_objective = objective_for_file(left);
        let right_objective = objective_for_file(right);
        let left_kind = kind_for_file(left, left_objective);
        let right_kind = kind_for_file(right, right_objective);
        let left_target = target_for_kind(left, left_kind);
        let right_target = target_for_kind(right, right_kind);
        let left_bias = learning.file_learning_bias(&left_target, &kind_label(left_kind));
        let right_bias = learning.file_learning_bias(&right_target, &kind_label(right_kind));
        right_bias
            .total_cmp(&left_bias)
            .then_with(|| left.cmp(right))
    });

    let mut plans = Vec::new();
    for file in safe_files.into_iter().take(8) {
        let evidence = graph
            .edges
            .iter()
            .filter(|edge| edge.to == format!("file:{file}"))
            .take(3)
            .map(|edge| edge.from.clone())
            .collect::<Vec<_>>();
        let objective = objective_for_file(&file);
        let mutation_kind = kind_for_file(&file, objective);
        let target_file = target_for_kind(&file, mutation_kind);
        if !task_allows(task, &target_file, objective, mutation_kind) {
            continue;
        }
        let regression_penalty = learning
            .file_learning_bias(&target_file, &kind_label(mutation_kind))
            .min(0.0)
            .abs();
        let success_bonus = learning
            .file_learning_bias(&target_file, &kind_label(mutation_kind))
            .max(0.0);
        let mut graph_evidence = evidence;
        if regression_penalty > 0.0 {
            graph_evidence.push(format!(
                "learning:regression_penalty:{:.2}",
                regression_penalty
            ));
        }
        if success_bonus > 0.0 {
            graph_evidence.push(format!("learning:success_bonus:{:.2}", success_bonus));
        }
        let expected_gain = (expected_gain(objective) + success_bonus * 0.05).clamp(0.0, 1.0);
        let estimated_risk = (0.12 + regression_penalty * 0.08).clamp(0.0, 1.0);
        if task.is_some_and(|value| estimated_risk > value.max_risk) {
            continue;
        }
        plans.push(MutationPlan {
            id: format!("plan:{}", file.replace('/', "_").replace('.', "_")),
            objective,
            target_file,
            mutation_kind,
            reason: format!("graph-guided safe improvement for {file}"),
            expected_gain,
            estimated_risk,
            evidence_weight: if graph_evidence.is_empty() { 0.0 } else { 0.2 },
            graph_evidence,
        });
    }
    plans.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(plans)
}

pub fn render_task_plans(memory_root: &str, task: &TaskContract) -> Result<String, String> {
    let plans = propose_mutation_plans_for_task(memory_root, Some(task))?;
    let learning = LearningContext::load(memory_root)?;
    let hypotheses = crate::evolution::rank_plans(&plans, &learning);
    if hypotheses.is_empty() {
        return Ok("(none)".to_string());
    }
    Ok(hypotheses
        .iter()
        .take(5)
        .map(|hypothesis| {
            format!(
                "{} objective={:?} target={} final_priority={:.2}",
                hypothesis.plan_id,
                hypothesis.objective,
                hypothesis.target_file,
                hypothesis.final_priority
            )
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

pub fn render_plans(memory_root: &str) -> Result<String, String> {
    let plans = propose_mutation_plans(memory_root)?;
    let learning = LearningContext::load(memory_root)?;
    let hypotheses = crate::evolution::rank_plans(&plans, &learning);
    if hypotheses.is_empty() {
        return Ok("(none)".to_string());
    }
    Ok(hypotheses
        .iter()
        .take(5)
        .map(|hypothesis| {
            let mut lines = vec![format!(
                "{} objective={:?} target={} final_priority={:.2} expected_gain={:.2} estimated_risk={:.2} regression_penalty={:.2} success_bonus={:.2} duplicate_penalty={:.2}",
                hypothesis.plan_id,
                hypothesis.objective,
                hypothesis.target_file,
                hypothesis.final_priority,
                hypothesis.expected_gain,
                hypothesis.estimated_risk,
                hypothesis.regression_penalty,
                hypothesis.success_bonus,
                hypothesis.duplicate_penalty
            )];
            lines.extend(
                hypothesis
                    .explanation
                    .iter()
                    .map(|line| format!("  - {line}")),
            );
            lines.join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

fn is_safe_target(file: &str) -> bool {
    file.starts_with("src/")
        && !file.starts_with("src/core/")
        && file != "src/main.rs"
        && file != "src/lib.rs"
        && file != "Cargo.toml"
        && !file.ends_with("/Cargo.toml")
}

fn objective_for_file(file: &str) -> MutationObjective {
    if file.contains("validator") {
        MutationObjective::ImproveValidation
    } else if file.contains("replay") || file.contains("promotion") {
        MutationObjective::ImproveReplayability
    } else if file.contains("metrics") || file.contains("learning") || file.contains("report") {
        MutationObjective::ImproveGraphMemory
    } else if file.contains("graph") {
        MutationObjective::ImproveGraphMemory
    } else if file.contains("test") {
        MutationObjective::ImproveTests
    } else {
        MutationObjective::ImproveReliability
    }
}

fn kind_for_file(file: &str, objective: MutationObjective) -> MutationKind {
    if file.contains("replay") {
        MutationKind::AddReplayAssertion
    } else if file.contains("metrics") || file.contains("learning") || file.contains("report") {
        match objective {
            MutationObjective::ImproveGraphMemory | MutationObjective::ImproveScoring => {
                MutationKind::AddMetricUpdate
            }
            MutationObjective::ReduceStorage => MutationKind::AddLearningSummaryField,
            _ => default_kind_for_objective(objective),
        }
    } else if file.contains("tests/") {
        MutationKind::AddUnitTest
    } else {
        default_kind_for_objective(objective)
    }
}

fn target_for_kind(file: &str, kind: MutationKind) -> String {
    match kind {
        MutationKind::AddTestSkeleton
        | MutationKind::AddUnitTest
        | MutationKind::AddReplayAssertion => "tests/evolution_generated_tests.rs".to_string(),
        _ => file.to_string(),
    }
}

fn expected_gain(objective: MutationObjective) -> f32 {
    match objective {
        MutationObjective::ImproveValidation | MutationObjective::ImproveReplayability => 0.55,
        MutationObjective::ImproveGraphMemory | MutationObjective::ImproveTests => 0.5,
        _ => 0.4,
    }
}

fn kind_label(kind: MutationKind) -> String {
    format!("{:?}", kind).to_ascii_lowercase()
}

fn task_allows(
    task: Option<&TaskContract>,
    target_file: &str,
    objective: MutationObjective,
    mutation_kind: MutationKind,
) -> bool {
    let Some(task) = task else {
        return true;
    };
    if !task.allowed_targets.is_empty()
        && !matches_target_patterns(target_file, &task.allowed_targets)
    {
        return false;
    }
    if matches_target_patterns(target_file, &task.forbidden_targets) {
        return false;
    }
    if !task.preferred_objectives.is_empty() && !task.preferred_objectives.contains(&objective) {
        return false;
    }
    if !task.allowed_mutation_kinds.is_empty()
        && !task.allowed_mutation_kinds.contains(&mutation_kind)
    {
        return false;
    }
    true
}
