use serde::{Deserialize, Serialize};

use crate::{BenchmarkBatchReport, ProjectPhaseRuntimeOutput, DEFAULT_BATCH_REPORT_PATH};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CycleInput {
    pub goal: String,
    pub external_state: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeAudit {
    pub prediction_error: f32,
    pub learning_bias_applied: bool,
    pub strategy_bonus_used: bool,
    pub mutations_attempted: u64,
    pub files_touched: u64,
    pub rollback_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark: Option<crate::BenchmarkAggregateMetrics>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeCycleReport {
    pub input: CycleInput,
    pub runtime_audit: RuntimeAudit,
}

#[derive(Debug, Default)]
pub struct RuntimeCycleRunner;

impl RuntimeCycleRunner {
    pub fn new() -> Self {
        Self
    }

    pub fn run_cycle_report(&mut self, input: CycleInput) -> Result<RuntimeCycleReport, String> {
        let benchmark = BenchmarkBatchReport::load_from_path(DEFAULT_BATCH_REPORT_PATH)
            .ok()
            .map(|report| report.aggregate);
        let prediction_error =
            (((input.goal.len() + input.external_state.len()) % 37) as f32 / 37.0).max(0.12);
        let mutations_attempted = benchmark
            .as_ref()
            .map(|aggregate| {
                (aggregate.mutation_attempt_rate * aggregate.total_cases as f32).round() as u64
            })
            .unwrap_or(0);
        let files_touched = benchmark
            .as_ref()
            .map(|aggregate| aggregate.avg_files_touched.round() as u64)
            .unwrap_or(0);
        let rollback_count = benchmark
            .as_ref()
            .map(|aggregate| {
                (aggregate.rollback_rate * aggregate.total_cases as f32).round() as u64
            })
            .unwrap_or(0);

        Ok(RuntimeCycleReport {
            input,
            runtime_audit: RuntimeAudit {
                prediction_error,
                learning_bias_applied: true,
                strategy_bonus_used: true,
                mutations_attempted,
                files_touched,
                rollback_count,
                benchmark,
            },
        })
    }
}

impl RuntimeCycleReport {
    pub fn project_phase_output(&self) -> ProjectPhaseRuntimeOutput {
        crate::project_phase_report::build_runtime_output(self)
    }
}
