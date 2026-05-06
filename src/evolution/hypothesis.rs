use serde::{Deserialize, Serialize};

use crate::contracts::{MutationObjective, MutationPlan};
use crate::evolution::learning_context::LearningContext;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvolutionHypothesis {
    pub id: String,
    pub plan_id: String,
    pub objective: MutationObjective,
    pub target_file: String,
    pub expected_gain: f32,
    pub estimated_risk: f32,
    pub regression_penalty: f32,
    pub success_bonus: f32,
    pub duplicate_penalty: f32,
    pub evidence_weight: f32,
    pub final_priority: f32,
    pub explanation: Vec<String>,
    pub graph_evidence: Vec<String>,
}

pub fn rank_plans(plans: &[MutationPlan], learning: &LearningContext) -> Vec<EvolutionHypothesis> {
    let mut hypotheses = plans
        .iter()
        .map(|plan| {
            let regression_penalty = learning.regression_penalty_for(plan);
            let success_bonus = learning.success_bonus_for(plan);
            let duplicate_penalty = learning.duplicate_penalty_for(plan);
            let evidence_weight = learning.evidence_weight_for(plan);
            let final_priority = plan.expected_gain - plan.estimated_risk + evidence_weight
                - regression_penalty
                + success_bonus
                - duplicate_penalty;
            EvolutionHypothesis {
                id: format!("hypothesis:{}", plan.id),
                plan_id: plan.id.clone(),
                objective: plan.objective,
                target_file: plan.target_file.clone(),
                expected_gain: plan.expected_gain,
                estimated_risk: plan.estimated_risk,
                regression_penalty,
                success_bonus,
                duplicate_penalty,
                evidence_weight,
                final_priority,
                explanation: build_explanation(
                    plan,
                    regression_penalty,
                    success_bonus,
                    duplicate_penalty,
                    evidence_weight,
                    final_priority,
                ),
                graph_evidence: plan.graph_evidence.clone(),
            }
        })
        .collect::<Vec<_>>();
    hypotheses.sort_by(|left, right| {
        right
            .final_priority
            .total_cmp(&left.final_priority)
            .then_with(|| left.estimated_risk.total_cmp(&right.estimated_risk))
            .then_with(|| left.plan_id.cmp(&right.plan_id))
    });
    hypotheses
}

fn build_explanation(
    plan: &MutationPlan,
    regression_penalty: f32,
    success_bonus: f32,
    duplicate_penalty: f32,
    evidence_weight: f32,
    final_priority: f32,
) -> Vec<String> {
    let mut explanation = vec![format!(
        "base={:.2} gain={:.2} risk={:.2} evidence={:.2}",
        plan.expected_gain - plan.estimated_risk + plan.evidence_weight,
        plan.expected_gain,
        plan.estimated_risk,
        evidence_weight
    )];
    if regression_penalty > 0.0 {
        explanation.push(format!(
            "regression penalty {:.2} from prior failures on {}",
            regression_penalty, plan.target_file
        ));
    }
    if success_bonus > 0.0 {
        explanation.push(format!(
            "success bonus {:.2} from retained candidate history",
            success_bonus
        ));
    }
    if duplicate_penalty > 0.0 {
        explanation.push(format!(
            "duplicate penalty {:.2} from seen mutation digest",
            duplicate_penalty
        ));
    }
    explanation.push(format!("final priority {:.2}", final_priority));
    explanation
}
