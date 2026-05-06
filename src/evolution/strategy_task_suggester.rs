use std::fs;
use std::path::Path;

use crate::contracts::{DeniedMutationKind, MutationKind, MutationObjective, TaskContract};
use crate::evolution::{load_corpus_patterns, load_corpus_summary};

pub fn suggest_strategy_tasks(
    memory_root: &str,
    corpus_id: &str,
) -> Result<Vec<TaskContract>, String> {
    let summary = load_corpus_summary(memory_root, corpus_id)?;
    let patterns = load_corpus_patterns(memory_root, corpus_id)?;
    let mut tasks = Vec::new();

    if patterns
        .detected_patterns
        .iter()
        .any(|pattern| pattern == "test_assertion_pattern")
    {
        tasks.push(build_task(
            corpus_id,
            "test_expansion",
            "Локальное расширение тестового покрытия",
            "Усилить безопасные test/replay паттерны на основе локального corpus.",
            vec!["tests/*".to_string()],
            vec![
                MutationObjective::ImproveTests,
                MutationObjective::ImproveReplayability,
            ],
            vec![MutationKind::AddUnitTest, MutationKind::AddReplayAssertion],
            2,
            0.20,
            7.0,
        ));
    }
    if patterns
        .detected_patterns
        .iter()
        .any(|pattern| pattern == "validation_guard_pattern")
    {
        tasks.push(build_task(
            corpus_id,
            "validation_hardening",
            "Локальное усиление validation guard",
            "Усилить validator safety через безопасные test/replay мутации.",
            vec!["tests/*".to_string()],
            vec![MutationObjective::ImproveValidation],
            vec![MutationKind::AddReplayAssertion, MutationKind::AddUnitTest],
            2,
            0.18,
            7.0,
        ));
    }
    if patterns
        .detected_patterns
        .iter()
        .any(|pattern| pattern == "report_writer_pattern")
    {
        tasks.push(build_task(
            corpus_id,
            "metrics_reporting",
            "Локальное усиление reporting/metrics",
            "Улучшить локальные метрики и learning summary без изменения core/runtime safety barriers.",
            vec!["src/evolution/*".to_string()],
            vec![MutationObjective::ImproveGraphMemory],
            vec![MutationKind::AddLearningSummaryField, MutationKind::AddMetricUpdate],
            2,
            0.18,
            7.0,
        ));
    }
    if patterns
        .detected_patterns
        .iter()
        .any(|pattern| pattern == "result_error_pattern")
    {
        tasks.push(build_task(
            corpus_id,
            "regression_avoidance",
            "Локальное усиление regression avoidance",
            "Усилить replay/error safety и избегать рискованных runtime target через tests/reporting.",
            vec!["tests/*".to_string(), "src/evolution/*".to_string()],
            vec![MutationObjective::ImproveReplayability, MutationObjective::ImproveReliability],
            vec![MutationKind::AddReplayAssertion, MutationKind::AddMetricUpdate],
            2,
            0.20,
            7.0,
        ));
    }

    if tasks.is_empty() {
        tasks.push(build_task(
            corpus_id,
            "safe_local_baseline",
            "Безопасная локальная baseline задача",
            "Выполнить только безопасные test/reporting улучшения по итогам локального corpus.",
            vec!["tests/*".to_string(), "src/evolution/*".to_string()],
            vec![MutationObjective::ImproveReliability],
            vec![MutationKind::AddUnitTest, MutationKind::AddMetricUpdate],
            1,
            0.18,
            7.0,
        ));
    }

    for task in &tasks {
        persist_task(memory_root, task)?;
    }
    let _ = summary;
    Ok(tasks)
}

pub fn list_suggested_tasks(memory_root: &str) -> Result<Vec<String>, String> {
    let dir = Path::new(memory_root).join("tasks").join("suggested");
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut ids = fs::read_dir(dir)
        .map_err(|error| format!("failed to read suggested tasks dir: {error}"))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry
                .path()
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
        })
        .filter_map(|name| name.strip_suffix(".task.json").map(str::to_string))
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    Ok(ids)
}

fn build_task(
    corpus_id: &str,
    suffix: &str,
    title_ru: &str,
    goal_ru: &str,
    allowed_targets: Vec<String>,
    preferred_objectives: Vec<MutationObjective>,
    allowed_mutation_kinds: Vec<MutationKind>,
    cycles: usize,
    max_risk: f32,
    min_score: f32,
) -> TaskContract {
    TaskContract {
        task_id: format!("corpus_{corpus_id}_{suffix}"),
        title_ru: title_ru.to_string(),
        goal_ru: goal_ru.to_string(),
        allowed_targets,
        forbidden_targets: vec![
            "src/core/*".to_string(),
            "src/main.rs".to_string(),
            "src/lib.rs".to_string(),
            "Cargo.toml".to_string(),
        ],
        preferred_objectives,
        allowed_mutation_kinds,
        denied_mutation_kinds: vec![
            DeniedMutationKind::DeleteCode,
            DeniedMutationKind::RewriteFunction,
            DeniedMutationKind::FreeDiff,
            DeniedMutationKind::DependencyAdd,
        ],
        cycles,
        require_replay: true,
        require_benchmark: false,
        require_russian_report: true,
        auto_promote: false,
        max_risk,
        min_score,
        source_corpus_id: Some(corpus_id.to_string()),
        created_at: crate::evolution::memory::now_unix(),
    }
}

fn persist_task(memory_root: &str, task: &TaskContract) -> Result<(), String> {
    let dir = Path::new(memory_root).join("tasks").join("suggested");
    fs::create_dir_all(&dir)
        .map_err(|error| format!("failed to create suggested tasks dir: {error}"))?;
    crate::evolution::memory::write_json(dir.join(format!("{}.task.json", task.task_id)), task)?;
    fs::write(
        dir.join(format!("{}.ru.md", task.task_id)),
        format!(
            "# Suggested Task EVA\n\ntitle: {}\ngoal: {}\nsource_corpus_id: {}\nauto_promote=false\nallowed_targets: {}\nallowed_mutation_kinds: {}\n",
            task.title_ru,
            task.goal_ru,
            task.source_corpus_id.as_deref().unwrap_or("нет"),
            task.allowed_targets.join(", "),
            task.allowed_mutation_kinds
                .iter()
                .map(|kind| format!("{kind:?}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    )
    .map_err(|error| format!("failed to write suggested task markdown: {error}"))
}
