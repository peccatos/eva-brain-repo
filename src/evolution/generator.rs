use crate::contracts::{
    MutationContract, MutationKind, MutationObjective, MutationPlan, RecombinedHypothesis,
};
use crate::evolution::templates::{
    generate_add_learning_summary_field, generate_add_metric_update, generate_add_replay_assertion,
    generate_add_unit_test,
};

pub fn generate_safe_mutation() -> MutationContract {
    MutationContract {
        id: "phase1-append-runtime-note".to_string(),
        kind: MutationKind::AppendComment,
        target_file: "src/runtime_cycle.rs".to_string(),
        search: None,
        replace: None,
        append: Some("// EVA Phase 1 sandbox-only mutation probe.".to_string()),
        reason: "prove bounded sandbox mutation without touching core project".to_string(),
        expected_gain: 0.05,
        risk: 0.1,
    }
}

pub fn generate_from_plan(plan: &MutationPlan) -> MutationContract {
    match plan.mutation_kind {
        MutationKind::AddUnitTest => return generate_add_unit_test(plan),
        MutationKind::AddReplayAssertion => return generate_add_replay_assertion(plan),
        MutationKind::AddLearningSummaryField => return generate_add_learning_summary_field(plan),
        MutationKind::AddMetricUpdate => return generate_add_metric_update(plan),
        MutationKind::AddTestSkeleton => {
            return generate_add_unit_test(plan);
        }
        MutationKind::AddMetricField => {
            return generate_add_metric_update(plan);
        }
        _ => {}
    }

    let (search, replace, append) = match plan.mutation_kind {
        MutationKind::AppendComment => (
            None,
            None,
            Some(format!(
                "// EVA planned note: {}.",
                safe_reason_fragment(&plan.reason)
            )),
        ),
        MutationKind::ReplaceText => (
            Some("// EVA Phase 1 sandbox-only mutation probe.".to_string()),
            Some("// EVA graph-guided sandbox-only mutation probe.".to_string()),
            None,
        ),
        MutationKind::ParameterTune => (
            Some("risk: 0.1".to_string()),
            Some("risk: 0.09".to_string()),
            None,
        ),
        MutationKind::AddTestSkeleton
        | MutationKind::AddMetricField
        | MutationKind::AddUnitTest
        | MutationKind::AddReplayAssertion
        | MutationKind::AddLearningSummaryField
        | MutationKind::AddMetricUpdate => unreachable!("handled by bounded templates"),
    };

    MutationContract {
        id: format!("mutation:{}", plan.id),
        kind: plan.mutation_kind,
        target_file: plan.target_file.clone(),
        search,
        replace,
        append,
        reason: format!(
            "planned {:?} from graph evidence: {}",
            plan.objective,
            plan.graph_evidence.join(",")
        ),
        expected_gain: plan.expected_gain.clamp(0.0, 1.0),
        risk: plan.estimated_risk.clamp(0.0, 1.0),
    }
}

pub fn generate_from_recombined_hypothesis(
    hypothesis: &RecombinedHypothesis,
) -> Result<MutationContract, String> {
    crate::evolution::recombination::generate_from_recombined_hypothesis(hypothesis)
}

pub fn default_kind_for_objective(objective: MutationObjective) -> MutationKind {
    match objective {
        MutationObjective::ImproveTests | MutationObjective::ImproveValidation => {
            MutationKind::AddUnitTest
        }
        MutationObjective::ImproveReplayability => MutationKind::AddReplayAssertion,
        MutationObjective::ImproveGraphMemory | MutationObjective::ImproveScoring => {
            MutationKind::AddMetricUpdate
        }
        MutationObjective::ReduceStorage => MutationKind::AddLearningSummaryField,
        MutationObjective::ImproveReliability | MutationObjective::ReduceRuntimeCost => {
            MutationKind::AddUnitTest
        }
    }
}

fn safe_reason_fragment(reason: &str) -> String {
    reason
        .chars()
        .filter(|ch| {
            ch.is_ascii_alphanumeric() || ch.is_ascii_whitespace() || *ch == '-' || *ch == '_'
        })
        .take(120)
        .collect::<String>()
        .trim()
        .to_string()
}
