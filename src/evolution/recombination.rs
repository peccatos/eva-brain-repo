use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::contracts::{
    EvolutionLogEntry, MutationContract, MutationKind, MutationObjective, MutationPlan,
    RecombinedHypothesis,
};
use crate::evolution::{generator, EvolutionMetrics, RegressionEntry, SuccessPatternEntry};
use crate::graph::{load_graph, EvolutionGraph};

const GENERATED_TEST_TARGET: &str = "tests/evolution_generated_tests.rs";

#[derive(Debug, Clone, Deserialize, Default)]
struct DistilledSummaryCompat {
    #[serde(default)]
    top_successful_mutation_kinds: Vec<DistilledCount>,
    #[serde(default)]
    risky_target_files: Vec<DistilledRiskyFile>,
    #[serde(default)]
    preferred_objectives: Vec<DistilledCount>,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct DistilledCount {
    key: String,
    count: u64,
    average_score: f32,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct DistilledRiskyFile {
    target_file: String,
    penalty: f32,
}

#[derive(Debug, Clone)]
struct CandidateFile {
    file: String,
    regression_penalty: f32,
    source_patterns: Vec<String>,
}

pub fn load_recombined_hypotheses(memory_root: &str) -> Result<Vec<RecombinedHypothesis>, String> {
    let distilled = load_distilled_summary(memory_root)?;
    let successes = load_success_entries(memory_root)?;
    let regressions = load_regression_entries(memory_root)?;
    let graph = load_graph(&Path::new(memory_root).join("graph.json"))?;
    let metrics = load_local_metrics(memory_root)?;

    let success_kind_stats = collect_success_kind_stats(&successes, &distilled);
    let preferred_objectives = collect_preferred_objectives(memory_root, &distilled, &metrics)?;
    let risky_file_penalties = collect_risky_file_penalties(&regressions, &distilled);
    let candidates = collect_candidate_files(&graph, &risky_file_penalties);

    let mut hypotheses = candidates
        .into_iter()
        .filter_map(|candidate| {
            build_hypothesis(
                candidate,
                &success_kind_stats,
                &preferred_objectives,
                metrics.promoted_count,
            )
        })
        .collect::<Vec<_>>();
    hypotheses.sort_by(|left, right| {
        right
            .confidence
            .total_cmp(&left.confidence)
            .then_with(|| right.expected_gain.total_cmp(&left.expected_gain))
            .then_with(|| left.estimated_risk.total_cmp(&right.estimated_risk))
            .then_with(|| left.hypothesis_id.cmp(&right.hypothesis_id))
    });
    Ok(hypotheses)
}

pub fn render_recombined_hypotheses(memory_root: &str) -> Result<String, String> {
    let hypotheses = load_recombined_hypotheses(memory_root)?;
    if hypotheses.is_empty() {
        return Ok("(none)".to_string());
    }
    Ok(hypotheses
        .iter()
        .take(5)
        .map(|hypothesis| {
            format!(
                "{} target={} kind={} confidence={:.2} expected_gain={:.2} estimated_risk={:.2} source_patterns={} avoided_risks={}",
                hypothesis.hypothesis_id,
                hypothesis.suggested_target,
                hypothesis.suggested_mutation_kind,
                hypothesis.confidence,
                hypothesis.expected_gain,
                hypothesis.estimated_risk,
                hypothesis.source_patterns.join(","),
                hypothesis.avoided_risks.join(",")
            )
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

pub fn top_recombined_hypothesis(memory_root: &str) -> Result<RecombinedHypothesis, String> {
    load_recombined_hypotheses(memory_root)?
        .into_iter()
        .next()
        .ok_or_else(|| "no recombined hypotheses available".to_string())
}

pub fn generate_from_recombined_hypothesis(
    hypothesis: &RecombinedHypothesis,
) -> Result<MutationContract, String> {
    let kind = mutation_kind_from_label(&hypothesis.suggested_mutation_kind)?;
    let plan = MutationPlan {
        id: hypothesis.hypothesis_id.clone(),
        objective: objective_from_label(&hypothesis.target_objective),
        target_file: hypothesis.suggested_target.clone(),
        mutation_kind: kind,
        reason: hypothesis.reason_ru.clone(),
        expected_gain: hypothesis.expected_gain.clamp(0.0, 1.0),
        estimated_risk: hypothesis.estimated_risk.clamp(0.0, 0.5),
        evidence_weight: hypothesis.confidence.clamp(0.0, 1.0),
        graph_evidence: hypothesis.source_patterns.clone(),
    };
    let mut mutation = generator::generate_from_plan(&plan);
    mutation.id = format!("mutation:{}", hypothesis.hypothesis_id);
    mutation.reason = format!(
        "recombined hypothesis {}: {}",
        hypothesis.hypothesis_id, hypothesis.reason_ru
    );
    mutation.kind = kind;
    mutation.target_file = hypothesis.suggested_target.clone();
    mutation.expected_gain = hypothesis.expected_gain.clamp(0.0, 1.0);
    mutation.risk = hypothesis.estimated_risk.clamp(0.0, 0.5);
    Ok(mutation)
}

fn load_distilled_summary(memory_root: &str) -> Result<DistilledSummaryCompat, String> {
    let path = Path::new(memory_root)
        .join("patterns")
        .join("local_distilled_patterns.json");
    if !path.exists() {
        return Ok(DistilledSummaryCompat::default());
    }
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read distilled patterns: {error}"))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("failed to parse distilled patterns: {error}"))
}

fn load_success_entries(memory_root: &str) -> Result<Vec<SuccessPatternEntry>, String> {
    let path = Path::new(memory_root).join("success_patterns.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read success patterns: {error}"))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("failed to parse success patterns: {error}"))
}

fn load_regression_entries(memory_root: &str) -> Result<Vec<RegressionEntry>, String> {
    let path = Path::new(memory_root).join("regressions.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents =
        fs::read_to_string(path).map_err(|error| format!("failed to read regressions: {error}"))?;
    serde_json::from_str(&contents).map_err(|error| format!("failed to parse regressions: {error}"))
}

fn load_local_metrics(memory_root: &str) -> Result<EvolutionMetrics, String> {
    let path = Path::new(memory_root).join("metrics.json");
    if !path.exists() {
        return Ok(EvolutionMetrics::default());
    }
    let contents =
        fs::read_to_string(path).map_err(|error| format!("failed to read metrics: {error}"))?;
    serde_json::from_str(&contents).map_err(|error| format!("failed to parse metrics: {error}"))
}

fn collect_success_kind_stats(
    successes: &[SuccessPatternEntry],
    distilled: &DistilledSummaryCompat,
) -> BTreeMap<String, (u64, f32)> {
    let mut stats = BTreeMap::new();
    for entry in successes {
        let slot = stats
            .entry(entry.mutation_kind.to_ascii_lowercase())
            .or_insert((0, 0.0));
        slot.0 += entry.success_count;
        slot.1 += entry.average_score * entry.success_count as f32;
    }
    for entry in &distilled.top_successful_mutation_kinds {
        let slot = stats
            .entry(entry.key.to_ascii_lowercase())
            .or_insert((0, 0.0));
        slot.0 += entry.count;
        slot.1 += entry.average_score * entry.count as f32;
    }
    stats
}

fn collect_preferred_objectives(
    memory_root: &str,
    distilled: &DistilledSummaryCompat,
    metrics: &EvolutionMetrics,
) -> Result<BTreeMap<String, u64>, String> {
    let mut objectives = BTreeMap::new();
    for entry in &distilled.preferred_objectives {
        *objectives.entry(entry.key.clone()).or_insert(0) += entry.count;
    }
    let path = Path::new(memory_root).join("evolution.jsonl");
    if path.exists() {
        let contents = fs::read_to_string(path)
            .map_err(|error| format!("failed to read evolution log: {error}"))?;
        for line in contents.lines().filter(|line| !line.trim().is_empty()) {
            if let Ok(entry) = serde_json::from_str::<EvolutionLogEntry>(line) {
                if entry.useful_change {
                    if let Some(objective) = entry.objective {
                        *objectives.entry(objective).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    if objectives.is_empty() && metrics.candidate_count > 0 {
        objectives.insert("ImproveReliability".to_string(), metrics.candidate_count);
    }
    Ok(objectives)
}

fn collect_risky_file_penalties(
    regressions: &[RegressionEntry],
    distilled: &DistilledSummaryCompat,
) -> BTreeMap<String, f32> {
    let mut penalties = BTreeMap::new();
    for entry in regressions {
        penalties
            .entry(entry.target_file.clone())
            .and_modify(|value: &mut f32| *value = (*value).max(entry.penalty))
            .or_insert(entry.penalty);
    }
    for entry in &distilled.risky_target_files {
        penalties
            .entry(entry.target_file.clone())
            .and_modify(|value: &mut f32| *value = (*value).max(entry.penalty))
            .or_insert(entry.penalty);
    }
    penalties
}

fn collect_candidate_files(
    graph: &EvolutionGraph,
    risky_file_penalties: &BTreeMap<String, f32>,
) -> Vec<CandidateFile> {
    let mut file_sources: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for edge in &graph.edges {
        if let Some(file) = edge.to.strip_prefix("file:") {
            if is_forbidden_target(file) || !file.starts_with("src/") {
                continue;
            }
            file_sources
                .entry(file.to_string())
                .or_default()
                .insert(edge.from.clone());
        }
    }
    for node in &graph.nodes {
        if let Some(file) = node.id.strip_prefix("file:") {
            if is_forbidden_target(file) || !file.starts_with("src/") {
                continue;
            }
            file_sources.entry(file.to_string()).or_default();
        }
    }

    let mut files = file_sources
        .into_iter()
        .map(|(file, sources)| CandidateFile {
            regression_penalty: risky_file_penalties.get(&file).copied().unwrap_or(0.0),
            file,
            source_patterns: sources.into_iter().collect(),
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.file.cmp(&right.file));
    files
}

fn build_hypothesis(
    candidate: CandidateFile,
    success_kind_stats: &BTreeMap<String, (u64, f32)>,
    preferred_objectives: &BTreeMap<String, u64>,
    promoted_count: u64,
) -> Option<RecombinedHypothesis> {
    let target_objective = preferred_objective_for_file(&candidate.file, preferred_objectives);
    let (suggested_mutation_kind, suggested_target, reason_ru, kind_bonus) =
        suggest_mutation_for_file(&candidate.file, &target_objective, success_kind_stats)?;
    if forbidden_kind(&suggested_mutation_kind) || is_forbidden_target(&suggested_target) {
        return None;
    }

    let objective_history = preferred_objectives
        .get(&target_objective)
        .copied()
        .unwrap_or(0) as f32;
    let success_count = success_kind_stats
        .get(&suggested_mutation_kind)
        .map(|entry| entry.0)
        .unwrap_or(0) as f32;
    let estimated_risk = estimate_risk(
        &candidate.file,
        &suggested_target,
        candidate.regression_penalty,
    );
    let expected_gain = (0.35
        + success_count.min(8.0) * 0.04
        + objective_history.min(5.0) * 0.03
        + kind_bonus
        + promoted_count.min(2) as f32 * 0.02
        - estimated_risk * 0.15)
        .clamp(0.0, 1.0);
    let confidence = (0.3
        + success_count.min(8.0) * 0.05
        + objective_history.min(5.0) * 0.04
        + candidate.source_patterns.len().min(4) as f32 * 0.03
        - candidate.regression_penalty * 0.05)
        .clamp(0.0, 1.0);

    let mut avoided_risks = Vec::new();
    if candidate.regression_penalty > 0.0 {
        avoided_risks.push(format!(
            "regression_penalty:{:.2}:{}",
            candidate.regression_penalty, candidate.file
        ));
    }
    if candidate.file.starts_with("src/runtime") || candidate.file.starts_with("src/evolution/") {
        avoided_risks.push(format!("runtime_to_tests_redirect:{}", candidate.file));
    }
    avoided_risks.sort();
    avoided_risks.dedup();

    let mut source_patterns = candidate.source_patterns;
    if source_patterns.is_empty() {
        source_patterns.push(format!("file:{}", candidate.file));
    }
    if success_count > 0.0 {
        source_patterns.push(format!(
            "success_kind:{}:{}",
            suggested_mutation_kind, success_count as u64
        ));
    }
    source_patterns.sort();
    source_patterns.dedup();

    Some(RecombinedHypothesis {
        hypothesis_id: format!(
            "recombined:{}:{}:{}",
            sanitize_id(&candidate.file),
            suggested_mutation_kind,
            sanitize_id(&suggested_target)
        ),
        source_patterns,
        avoided_risks,
        target_objective,
        suggested_mutation_kind,
        suggested_target,
        reason_ru,
        expected_gain,
        estimated_risk,
        confidence,
    })
}

fn preferred_objective_for_file(
    file: &str,
    preferred_objectives: &BTreeMap<String, u64>,
) -> String {
    let inferred = if file.contains("validator") {
        "ImproveValidation"
    } else if file.contains("replay") || file.contains("promotion") {
        "ImproveReplayability"
    } else if file.contains("metrics") || file.contains("learning") || file.contains("report") {
        "ImproveGraphMemory"
    } else {
        "ImproveReliability"
    };
    if preferred_objectives.is_empty() {
        return inferred.to_string();
    }
    preferred_objectives
        .iter()
        .max_by(|left, right| left.1.cmp(right.1).then_with(|| right.0.cmp(left.0)))
        .map(|entry| entry.0.clone())
        .unwrap_or_else(|| inferred.to_string())
}

fn suggest_mutation_for_file(
    file: &str,
    target_objective: &str,
    success_kind_stats: &BTreeMap<String, (u64, f32)>,
) -> Option<(String, String, String, f32)> {
    let add_unit_success = success_kind_stats
        .get("addunittest")
        .map(|entry| entry.0)
        .unwrap_or(0);
    let add_replay_success = success_kind_stats
        .get("addreplayassertion")
        .map(|entry| entry.0)
        .unwrap_or(0);
    let metric_update_success = success_kind_stats
        .get("addmetricupdate")
        .map(|entry| entry.0)
        .unwrap_or(0);
    let learning_success = success_kind_stats
        .get("addlearningsummaryfield")
        .map(|entry| entry.0)
        .unwrap_or(0);

    if file.contains("validator") {
        return Some((
            "addunittest".to_string(),
            GENERATED_TEST_TARGET.to_string(),
            "Рекомбинация выбрала усиление validator safety через локально успешный тестовый паттерн.".to_string(),
            0.12,
        ));
    }
    if file.starts_with("src/runtime") || file.starts_with("src/evolution/") {
        if add_replay_success >= add_unit_success && add_replay_success > 0 {
            return Some((
                "addreplayassertion".to_string(),
                GENERATED_TEST_TARGET.to_string(),
                "Рекомбинация отвела рискованный runtime/evolution target в replay/test слой и использовала успешный replay-паттерн.".to_string(),
                0.14,
            ));
        }
        if add_unit_success > 0 {
            return Some((
                "addunittest".to_string(),
                GENERATED_TEST_TARGET.to_string(),
                "Рекомбинация отвела рискованный runtime/evolution target в безопасный test target и использовала успешный AddUnitTest-паттерн.".to_string(),
                0.14,
            ));
        }
    }
    if file.contains("metrics") || file.contains("report") || file.contains("learning") {
        if learning_success > metric_update_success {
            return Some((
                "addlearningsummaryfield".to_string(),
                "src/evolution/metrics.rs".to_string(),
                "Рекомбинация объединила успешную reporting/learning историю в компактное расширение summary-поля.".to_string(),
                0.1,
            ));
        }
        return Some((
            "addmetricupdate".to_string(),
            "src/evolution/metrics.rs".to_string(),
            "Рекомбинация объединила успешную metrics/reporting историю в безопасное обновление метрик.".to_string(),
            0.1,
        ));
    }
    if target_objective == "ImproveValidation" && add_unit_success > 0 {
        return Some((
            "addunittest".to_string(),
            GENERATED_TEST_TARGET.to_string(),
            "Рекомбинация усилила validation area через успешный локальный unit-test паттерн."
                .to_string(),
            0.1,
        ));
    }
    if add_unit_success > 0 {
        return Some((
            "addunittest".to_string(),
            GENERATED_TEST_TARGET.to_string(),
            "Рекомбинация предпочла наиболее успешный локальный AddUnitTest-паттерн перед прямой risky mutation.".to_string(),
            0.08,
        ));
    }
    Some((
        "addmetricupdate".to_string(),
        "src/evolution/metrics.rs".to_string(),
        "Рекомбинация выбрала детерминированное безопасное обновление метрик как fallback без прямого риска для core.".to_string(),
        0.04,
    ))
}

fn estimate_risk(file: &str, target: &str, regression_penalty: f32) -> f32 {
    let mut risk = 0.08 + regression_penalty * 0.08;
    if file.starts_with("src/runtime") || file.starts_with("src/evolution/") {
        risk += 0.05;
    }
    if target.starts_with("tests/") {
        risk -= 0.05;
    }
    if target == "src/evolution/metrics.rs" {
        risk += 0.01;
    }
    risk.clamp(0.02, 0.45)
}

fn is_forbidden_target(file: &str) -> bool {
    file.starts_with("src/core/")
        || file == "src/main.rs"
        || file == "src/lib.rs"
        || file == "Cargo.toml"
        || file.ends_with("/Cargo.toml")
}

fn forbidden_kind(kind: &str) -> bool {
    matches!(
        kind,
        "appendcomment" | "deletecode" | "rewritefunction" | "freediff" | "dependencyadd"
    )
}

fn mutation_kind_from_label(label: &str) -> Result<MutationKind, String> {
    match label {
        "addunittest" => Ok(MutationKind::AddUnitTest),
        "addreplayassertion" => Ok(MutationKind::AddReplayAssertion),
        "addlearningsummaryfield" => Ok(MutationKind::AddLearningSummaryField),
        "addmetricupdate" => Ok(MutationKind::AddMetricUpdate),
        _ => Err(format!("unsupported recombined mutation kind: {label}")),
    }
}

fn objective_from_label(label: &str) -> MutationObjective {
    match label {
        "ImproveTests" => MutationObjective::ImproveTests,
        "ImproveValidation" => MutationObjective::ImproveValidation,
        "ImproveReplayability" => MutationObjective::ImproveReplayability,
        "ImproveGraphMemory" => MutationObjective::ImproveGraphMemory,
        "ImproveScoring" => MutationObjective::ImproveScoring,
        "ReduceStorage" => MutationObjective::ReduceStorage,
        "ReduceRuntimeCost" => MutationObjective::ReduceRuntimeCost,
        _ => MutationObjective::ImproveReliability,
    }
}

fn sanitize_id(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_ascii_lowercase()
}
