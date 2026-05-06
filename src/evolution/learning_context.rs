use crate::contracts::MutationPlan;
use crate::evolution::dedup::{compute_mutation_digest, load_dedup_entries, DedupEntry};
use crate::evolution::generator::generate_from_plan;
use crate::evolution::metrics::{load_metrics, EvolutionMetrics};
use crate::evolution::regression_memory::{load_regressions, target_area, RegressionEntry};
use crate::evolution::success_memory::{load_success_patterns, SuccessPatternEntry};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LearningContext {
    pub regressions: Vec<RegressionEntry>,
    pub successes: Vec<SuccessPatternEntry>,
    pub dedup_entries: Vec<DedupEntry>,
    pub metrics: EvolutionMetrics,
}

impl LearningContext {
    pub fn load(memory_root: &str) -> Result<Self, String> {
        Ok(Self {
            regressions: load_regressions(memory_root)?,
            successes: load_success_patterns(memory_root)?,
            dedup_entries: load_dedup_entries(memory_root)?,
            metrics: load_metrics(memory_root).unwrap_or_default(),
        })
    }

    pub fn regression_penalty_for(&self, plan: &MutationPlan) -> f32 {
        let area = target_area(&plan.target_file);
        self.regressions
            .iter()
            .filter(|entry| {
                entry.target_file == plan.target_file
                    || (entry.target_area == area && entry.mutation_kind == mutation_kind(plan))
            })
            .map(|entry| entry.penalty)
            .fold(0.0_f32, f32::max)
    }

    pub fn success_bonus_for(&self, plan: &MutationPlan) -> f32 {
        let area = target_area(&plan.target_file);
        self.successes
            .iter()
            .filter(|entry| {
                entry.target_file == plan.target_file
                    || (entry.target_area == area && entry.mutation_kind == mutation_kind(plan))
            })
            .map(|entry| entry.bonus)
            .fold(0.0_f32, f32::max)
    }

    pub fn duplicate_penalty_for(&self, plan: &MutationPlan) -> f32 {
        let predicted = generate_from_plan(plan);
        let digest = compute_mutation_digest(&predicted);
        self.dedup_entries
            .iter()
            .find(|entry| entry.digest == digest)
            .map(duplicate_penalty)
            .unwrap_or(0.0)
    }

    pub fn evidence_weight_for(&self, plan: &MutationPlan) -> f32 {
        let evidence_bonus = (plan.graph_evidence.len() as f32 * 0.03).min(0.12);
        let runtime_bonus = if self.metrics.total_runs == 0 {
            0.0
        } else {
            let pass_ratio = self.metrics.passed_runs as f32 / self.metrics.total_runs as f32;
            pass_ratio.min(1.0) * 0.05
        };
        plan.evidence_weight + evidence_bonus + runtime_bonus
    }

    pub fn file_learning_bias(&self, target_file: &str, mutation_kind: &str) -> f32 {
        let area = target_area(target_file);
        let success = self
            .successes
            .iter()
            .filter(|entry| {
                entry.target_file == target_file
                    || (entry.target_area == area && entry.mutation_kind == mutation_kind)
            })
            .map(|entry| entry.bonus)
            .fold(0.0_f32, f32::max);
        let regression = self
            .regressions
            .iter()
            .filter(|entry| {
                entry.target_file == target_file
                    || (entry.target_area == area && entry.mutation_kind == mutation_kind)
            })
            .map(|entry| entry.penalty)
            .fold(0.0_f32, f32::max);
        success - regression
    }
}

fn mutation_kind(plan: &MutationPlan) -> String {
    format!("{:?}", plan.mutation_kind).to_ascii_lowercase()
}

fn duplicate_penalty(entry: &DedupEntry) -> f32 {
    if !entry.useful_change || entry.score < 5.0 {
        (1.0 + entry.seen_count as f32 * 0.25).min(3.0)
    } else {
        (0.2 + entry.seen_count as f32 * 0.05).min(0.6)
    }
}
