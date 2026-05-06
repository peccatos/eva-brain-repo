use serde::{Deserialize, Serialize};

use crate::benchmark_contract::BenchmarkFailureType;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkCaseMetrics {
    pub case_id: String,
    pub repo_full_name: String,
    pub failure_type: BenchmarkFailureType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_strategy: Option<String>,
    pub files_touched: u64,
    pub mutations_attempted: u64,
    pub rollback_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prediction_error_before: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prediction_error_after: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adjusted_error_improved: Option<bool>,
    pub learning_bias_applied: bool,
    pub github_context_used: bool,
    pub success: bool,
    pub unreproducible: bool,
    pub duration_ms: u64,
    pub candidate_files_found: u64,
    pub repair_block_reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct BenchmarkAggregateMetrics {
    pub total_cases: u64,
    pub reproducible_cases: u64,
    pub successful_fixes: u64,
    pub success_rate: f32,
    pub rollback_rate: f32,
    pub avg_files_touched: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avg_prediction_error_after: Option<f32>,
    pub github_context_usage_rate: f32,
    pub learning_active_rate: f32,
    pub mutation_attempt_rate: f32,
}

impl BenchmarkAggregateMetrics {
    pub fn from_cases(cases: &[BenchmarkCaseMetrics]) -> Self {
        let total_cases = cases.len() as u64;
        if total_cases == 0 {
            return Self::default();
        }

        let reproducible_cases = cases.iter().filter(|case| !case.unreproducible).count() as u64;
        let successful_fixes = cases.iter().filter(|case| case.success).count() as u64;
        let rollback_cases = cases.iter().filter(|case| case.rollback_count > 0).count() as u64;
        let mutation_cases = cases
            .iter()
            .filter(|case| case.mutations_attempted > 0)
            .count() as u64;
        let github_cases = cases.iter().filter(|case| case.github_context_used).count() as u64;
        let learning_cases = cases
            .iter()
            .filter(|case| case.learning_bias_applied)
            .count() as u64;
        let avg_files_touched = cases
            .iter()
            .map(|case| case.files_touched as f32)
            .sum::<f32>()
            / total_cases as f32;
        let prediction_values = cases
            .iter()
            .filter_map(|case| case.prediction_error_after)
            .collect::<Vec<_>>();
        let avg_prediction_error_after = if prediction_values.is_empty() {
            None
        } else {
            Some(prediction_values.iter().sum::<f32>() / prediction_values.len() as f32)
        };

        Self {
            total_cases,
            reproducible_cases,
            successful_fixes,
            success_rate: successful_fixes as f32 / total_cases as f32,
            rollback_rate: rollback_cases as f32 / total_cases as f32,
            avg_files_touched,
            avg_prediction_error_after,
            github_context_usage_rate: github_cases as f32 / total_cases as f32,
            learning_active_rate: learning_cases as f32 / total_cases as f32,
            mutation_attempt_rate: mutation_cases as f32 / total_cases as f32,
        }
    }
}
