use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::contracts::TaskContract;
use crate::evolution::{
    autonomy_status, ensure_strategy_portfolio, load_metrics, load_portfolio, load_regressions,
    load_success_patterns,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct EvolutionPolicy {
    pub selected_strategy: String,
    pub policy_reason_ru: String,
    pub allowed_mutation_kinds: Vec<String>,
    pub preferred_targets: Vec<String>,
    pub risk_limit: f32,
}

pub fn refresh_evolution_policy(
    project_root: &str,
    memory_root: &str,
    task: Option<&TaskContract>,
) -> Result<EvolutionPolicy, String> {
    let policy = build_policy(project_root, memory_root, task)?;
    crate::evolution::memory::write_json(
        Path::new(memory_root).join("evolution_policy.json"),
        &policy,
    )?;
    Ok(policy)
}

pub fn load_or_refresh_evolution_policy(
    project_root: &str,
    memory_root: &str,
    task: Option<&TaskContract>,
) -> Result<EvolutionPolicy, String> {
    let path = Path::new(memory_root).join("evolution_policy.json");
    if path.exists() {
        let contents = std::fs::read_to_string(&path)
            .map_err(|error| format!("failed to read evolution policy: {error}"))?;
        let parsed: EvolutionPolicy = serde_json::from_str(&contents)
            .map_err(|error| format!("failed to parse evolution policy: {error}"))?;
        if !parsed.selected_strategy.is_empty() {
            return Ok(parsed);
        }
    }
    refresh_evolution_policy(project_root, memory_root, task)
}

pub fn print_evolution_policy(
    project_root: &str,
    memory_root: &str,
    task: Option<&TaskContract>,
) -> Result<String, String> {
    let policy = load_or_refresh_evolution_policy(project_root, memory_root, task)?;
    Ok(serde_json::to_string_pretty(&policy).expect("serialize evolution policy"))
}

fn build_policy(
    project_root: &str,
    memory_root: &str,
    task: Option<&TaskContract>,
) -> Result<EvolutionPolicy, String> {
    let mutation_portfolio = load_portfolio(memory_root)?;
    let strategy_portfolio = ensure_strategy_portfolio(memory_root)?;
    let regressions = load_regressions(memory_root)?;
    let successes = load_success_patterns(memory_root)?;
    let autonomy = autonomy_status(project_root, memory_root)?;
    let metrics = load_metrics(memory_root)?;

    let addunittest_saturated = mutation_portfolio
        .kinds
        .iter()
        .find(|entry| entry.mutation_kind == "addunittest")
        .map(|entry| entry.saturation_score > 0.0 || entry.candidate_count > 0)
        .unwrap_or(false);
    let runtime_regressions = regressions
        .iter()
        .filter(|entry| {
            entry.target_file.starts_with("src/runtime")
                || entry.target_file.starts_with("src/evolution/")
        })
        .count();
    let metrics_success = successes
        .iter()
        .filter(|entry| {
            entry.mutation_kind == "addmetricupdate"
                || entry.mutation_kind == "addlearningsummaryfield"
        })
        .count();

    let mut strategy_scores = vec![
        ("TestExpansion", 0.2_f32),
        ("ReplaySafety", 0.2),
        ("MetricsReporting", 0.2),
        ("ValidationHardening", 0.2),
        ("RegressionAvoidance", 0.2),
        ("CandidateReview", 0.1),
    ];

    for (strategy, score) in &mut strategy_scores {
        if let Some(entry) = strategy_portfolio
            .strategies
            .iter()
            .find(|entry| entry.strategy == *strategy)
        {
            *score -= entry.saturation_score;
            if entry.seen_count == 0 {
                *score += 0.25;
            }
        } else {
            *score += 0.25;
        }
    }

    if addunittest_saturated {
        bump(&mut strategy_scores, "ReplaySafety", 0.25);
        bump(&mut strategy_scores, "MetricsReporting", 0.25);
        bump(&mut strategy_scores, "ValidationHardening", 0.10);
        bump(&mut strategy_scores, "TestExpansion", -0.20);
    }
    if runtime_regressions > 0 {
        bump(&mut strategy_scores, "RegressionAvoidance", 0.35);
        bump(&mut strategy_scores, "ReplaySafety", 0.15);
    }
    if metrics_success > 0 {
        bump(&mut strategy_scores, "MetricsReporting", 0.10);
    }
    if autonomy.current_safe_autonomy_level < 3 {
        bump(&mut strategy_scores, "ValidationHardening", 0.10);
    }
    if metrics.promoted_count >= 3 {
        bump(&mut strategy_scores, "RegressionAvoidance", 0.05);
    }

    let mut selected_strategy = strategy_scores[0].0.to_string();
    let mut selected_score = strategy_scores[0].1;
    for (strategy, score) in strategy_scores.into_iter().skip(1) {
        if score > selected_score
            || (score - selected_score).abs() < f32::EPSILON
                && strategy < selected_strategy.as_str()
        {
            selected_strategy = strategy.to_string();
            selected_score = score;
        }
    }

    let mut allowed_mutation_kinds = strategy_allowed_kinds(&selected_strategy);
    if let Some(task) = task {
        if !task.allowed_mutation_kinds.is_empty() {
            let allowed = task
                .allowed_mutation_kinds
                .iter()
                .map(|kind| format!("{kind:?}").to_ascii_lowercase())
                .collect::<Vec<_>>();
            allowed_mutation_kinds.retain(|kind| allowed.contains(kind));
        }
    }
    if allowed_mutation_kinds.is_empty() {
        allowed_mutation_kinds = vec![
            "addreplayassertion".to_string(),
            "addmetricupdate".to_string(),
            "addlearningsummaryfield".to_string(),
            "addunittest".to_string(),
        ];
    }

    let preferred_targets = strategy_preferred_targets(&selected_strategy);
    let risk_limit = if let Some(task) = task {
        task.max_risk
    } else if autonomy.current_safe_autonomy_level >= 3 {
        0.25
    } else {
        0.18
    };

    Ok(EvolutionPolicy {
        selected_strategy: selected_strategy.clone(),
        policy_reason_ru: format!(
            "Политика выбрала стратегию {}: saturation addunittest={}, runtime_regressions={}, metrics_success={}, autonomy_level={}, risk_limit={:.2}.",
            selected_strategy,
            addunittest_saturated,
            runtime_regressions,
            metrics_success,
            autonomy.current_safe_autonomy_level,
            risk_limit
        ),
        allowed_mutation_kinds,
        preferred_targets,
        risk_limit,
    })
}

fn strategy_allowed_kinds(strategy: &str) -> Vec<String> {
    match strategy {
        "ReplaySafety" => vec!["addreplayassertion".to_string(), "addunittest".to_string()],
        "MetricsReporting" => vec![
            "addmetricupdate".to_string(),
            "addlearningsummaryfield".to_string(),
        ],
        "ValidationHardening" => vec!["addunittest".to_string()],
        "RegressionAvoidance" => vec![
            "addreplayassertion".to_string(),
            "addunittest".to_string(),
            "addmetricupdate".to_string(),
        ],
        "CandidateReview" => vec![
            "addreplayassertion".to_string(),
            "addmetricupdate".to_string(),
        ],
        _ => vec!["addunittest".to_string(), "addreplayassertion".to_string()],
    }
}

fn strategy_preferred_targets(strategy: &str) -> Vec<String> {
    match strategy {
        "ReplaySafety" => vec!["tests/evolution_generated_tests.rs".to_string()],
        "MetricsReporting" => vec!["src/evolution/metrics.rs".to_string()],
        "ValidationHardening" => vec!["tests/evolution_generated_tests.rs".to_string()],
        "RegressionAvoidance" => vec![
            "tests/evolution_generated_tests.rs".to_string(),
            "src/evolution/metrics.rs".to_string(),
        ],
        "CandidateReview" => vec!["tests/evolution_generated_tests.rs".to_string()],
        _ => vec!["tests/evolution_generated_tests.rs".to_string()],
    }
}

fn bump(scores: &mut [(&str, f32)], strategy: &str, delta: f32) {
    if let Some((_, score)) = scores.iter_mut().find(|(name, _)| *name == strategy) {
        *score += delta;
    }
}
