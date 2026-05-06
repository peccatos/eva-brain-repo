use serde::{Deserialize, Serialize};

use crate::contracts::{sandbox_result::CommandResult, MutationKind};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvolutionScore {
    pub accepted: bool,
    pub score: f32,
    pub useful_change: bool,
    pub non_candidate_reason: Option<String>,
    pub check_passed: bool,
    pub test_passed: bool,
    pub run_passed: bool,
    pub total_duration_ms: u128,
}

pub fn score_cycle(
    mutation_kind: MutationKind,
    check: &CommandResult,
    test: &CommandResult,
    run: Option<&CommandResult>,
) -> EvolutionScore {
    let run_passed = run.map(|result| result.success).unwrap_or(false);
    let total_duration_ms =
        check.duration_ms + test.duration_ms + run.map(|result| result.duration_ms).unwrap_or(0);
    let mut score: f32 = 0.0;
    if check.success {
        score += 3.0;
    }
    if test.success {
        score += 4.0;
    }
    if run_passed {
        score += 3.0;
    }

    let (useful_template, non_candidate_reason) = mutation_usefulness(mutation_kind);
    let useful_change = useful_template && check.success && test.success && run_passed;
    if !useful_template {
        score = score.min(2.0);
    }

    EvolutionScore {
        accepted: check.success && test.success,
        score,
        useful_change,
        non_candidate_reason,
        check_passed: check.success,
        test_passed: test.success,
        run_passed,
        total_duration_ms,
    }
}

fn mutation_usefulness(mutation_kind: MutationKind) -> (bool, Option<String>) {
    match mutation_kind {
        MutationKind::AppendComment => (false, Some("cosmetic_mutation".to_string())),
        MutationKind::ReplaceText
        | MutationKind::ParameterTune
        | MutationKind::AddTestSkeleton
        | MutationKind::AddMetricField
        | MutationKind::AddUnitTest
        | MutationKind::AddReplayAssertion
        | MutationKind::AddLearningSummaryField
        | MutationKind::AddMetricUpdate => (true, None),
    }
}
