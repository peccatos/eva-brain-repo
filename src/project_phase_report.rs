use serde::{Deserialize, Serialize};

use crate::{RuntimeAudit, RuntimeCycleReport};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectPhaseStatus {
    Active,
    Blocked,
    Stable,
    Partial,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectPhaseReport {
    pub project_phase: String,
    pub phase_status: ProjectPhaseStatus,
    pub summary_ru: String,
    pub proven_capabilities: Vec<String>,
    pub unproven_capabilities: Vec<String>,
    pub current_blocker_ru: String,
    pub current_risk_ru: String,
    pub last_confirmed_result_ru: String,
    pub next_required_step_ru: String,
    pub recommended_mode_ru: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectPhaseRuntimeOutput {
    pub project_report_ru: ProjectPhaseReport,
    pub runtime_audit: RuntimeAudit,
}

pub fn build_runtime_output(report: &RuntimeCycleReport) -> ProjectPhaseRuntimeOutput {
    ProjectPhaseRuntimeOutput {
        project_report_ru: ProjectPhaseReport::from_runtime_audit(&report.runtime_audit),
        runtime_audit: report.runtime_audit.clone(),
    }
}

impl ProjectPhaseReport {
    pub fn from_runtime_audit(audit: &RuntimeAudit) -> Self {
        let (project_phase, phase_status) = classify_phase(audit);
        let proven_capabilities = build_proven_capabilities(audit);
        let unproven_capabilities = build_unproven_capabilities(audit);
        let summary_ru = match project_phase.as_str() {
            "benchmark_repair_activation" => "Система уже дошла до реальных мутаций на benchmark-кейсах, но устойчивые успешные фиксы ещё не доказаны.".to_string(),
            "benchmark_reproduction" => "Система умеет воспроизводить benchmark-кейсы, но путь ремонта пока слабее, чем путь диагностики.".to_string(),
            "repair_validation" => "Система уже умеет чинить часть кейсов, но стабильность repair loop ещё нужно усиливать.".to_string(),
            "stable_repair_loop" => "Ремонтный цикл подтверждён на серии реальных кейсов.".to_string(),
            _ => "Ядро EVA работает локально и готово к демонстрационному циклу.".to_string(),
        };
        let current_blocker_ru = if let Some(aggregate) = &audit.benchmark {
            if aggregate.success_rate == 0.0 && aggregate.mutation_attempt_rate > 0.0 {
                "Главный блокер — мутации уже идут, но практическая результативность фиксов пока нулевая.".to_string()
            } else if aggregate.reproducible_cases > 0 {
                "Главный блокер — benchmark-кейсы воспроизводятся, но repair loop ещё не даёт устойчивых побед.".to_string()
            } else {
                "Главный блокер — пока нет подтверждённой серии воспроизводимых кейсов для ремонта."
                    .to_string()
            }
        } else {
            "Главный блокер — benchmark-сигналы ещё не подгружены, поэтому система видит только локальный runtime срез.".to_string()
        };
        let current_risk_ru = if let Some(aggregate) = &audit.benchmark {
            format!(
                "Риск сейчас в том, что success rate {:.1}% отстаёт от доли mutation attempts {:.1}%.",
                aggregate.success_rate * 100.0,
                aggregate.mutation_attempt_rate * 100.0
            )
        } else {
            format!(
                "Риск сейчас в том, что есть только runtime-сигналы без benchmark-валидации; текущая ошибка предсказания {:.3}.",
                audit.prediction_error
            )
        };
        let last_confirmed_result_ru = if let Some(aggregate) = &audit.benchmark {
            format!(
                "Подтверждены runtime цикл, фазовый отчёт и benchmark-пайплайн: всего кейсов {}, воспроизводимых {}, мутаций {}.",
                aggregate.total_cases,
                aggregate.reproducible_cases,
                audit.mutations_attempted
            )
        } else {
            "Подтверждены локальный runtime cycle, русский фазовый отчёт и repo patch mode."
                .to_string()
        };
        let next_required_step_ru = if let Some(aggregate) = &audit.benchmark {
            if aggregate.success_rate > 0.0 {
                "Нужно увеличить число успешных фиксов и удержать валидацию без регрессий."
                    .to_string()
            } else {
                "Нужно получить хотя бы один успешный фикс на воспроизводимом кейсе и подтвердить это серией валидаций.".to_string()
            }
        } else {
            "Нужно запустить benchmark pipeline на локальном fixture или реальном репозитории и получить benchmark-метрики.".to_string()
        };
        let recommended_mode_ru = if audit.benchmark.is_some() {
            "Работать в benchmark режиме с ограниченным budget и явным контролем mutation/rollback."
                .to_string()
        } else {
            "Работать в локальном demo режиме и затем переходить к benchmark pipeline.".to_string()
        };

        Self {
            project_phase,
            phase_status,
            summary_ru,
            proven_capabilities,
            unproven_capabilities,
            current_blocker_ru,
            current_risk_ru,
            last_confirmed_result_ru,
            next_required_step_ru,
            recommended_mode_ru,
        }
    }
}

fn classify_phase(audit: &RuntimeAudit) -> (String, ProjectPhaseStatus) {
    if let Some(aggregate) = &audit.benchmark {
        if aggregate.success_rate >= 0.5 && aggregate.successful_fixes > 1 {
            return ("stable_repair_loop".to_string(), ProjectPhaseStatus::Stable);
        }
        if aggregate.success_rate > 0.0 {
            return ("repair_validation".to_string(), ProjectPhaseStatus::Active);
        }
        if aggregate.mutation_attempt_rate > 0.0 && audit.files_touched > 0 {
            return (
                "benchmark_repair_activation".to_string(),
                ProjectPhaseStatus::Partial,
            );
        }
        if aggregate.reproducible_cases > 0 {
            return (
                "benchmark_reproduction".to_string(),
                ProjectPhaseStatus::Partial,
            );
        }
    }

    ("core_runtime".to_string(), ProjectPhaseStatus::Active)
}

fn build_proven_capabilities(audit: &RuntimeAudit) -> Vec<String> {
    let mut capabilities = vec![
        "Русский фазовый отчёт работает".to_string(),
        "Локальный runtime cycle работает".to_string(),
        "Repo patch mode работает".to_string(),
    ];

    if let Some(aggregate) = &audit.benchmark {
        if aggregate.reproducible_cases > 0 {
            capabilities.push("Benchmark reproduction подтверждён".to_string());
        }
        if aggregate.mutation_attempt_rate > 0.0 {
            capabilities.push("Repair activation подтверждён".to_string());
        }
    }

    capabilities
}

fn build_unproven_capabilities(audit: &RuntimeAudit) -> Vec<String> {
    let mut capabilities = Vec::new();
    if let Some(aggregate) = &audit.benchmark {
        if aggregate.success_rate == 0.0 {
            capabilities.push("Стабильный успешный ремонт на реальных кейсах".to_string());
        }
        if aggregate.success_rate < 0.5 {
            capabilities.push("Надёжный массовый repair loop".to_string());
        }
    } else {
        capabilities.push("Benchmark reproduction на реальных кейсах".to_string());
        capabilities.push("Стабильный успешный ремонт на реальных кейсах".to_string());
    }
    capabilities.push("Фоновый daemon-режим".to_string());
    capabilities
}
