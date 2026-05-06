use std::fs;
use std::path::{Path, PathBuf};

use crate::contracts::{
    EvolutionLogEntry, EvolutionReport, EvolutionStatus, MutationContract, MutationKind,
};
use crate::evolution::{memory, ReplayResult};

#[derive(Debug, Clone)]
struct ReplayInfo {
    replay_ru: String,
    replay_status: String,
    replay_checked_at: Option<u64>,
}

pub fn write_report(
    memory_root: &str,
    entry: &EvolutionLogEntry,
    mutation: &MutationContract,
) -> Result<EvolutionReport, String> {
    let replay = load_replay_info(memory_root, &entry.run_id)?;
    let report = build_report(entry, mutation, replay);
    persist_report(memory_root, &report)?;
    Ok(report)
}

pub fn refresh_report(memory_root: &str, run_id: &str) -> Result<EvolutionReport, String> {
    let entry = load_log_entry(memory_root, run_id)?
        .or_else(|| load_candidate_entry(memory_root, run_id).ok().flatten())
        .ok_or_else(|| format!("no evolution data found for report {run_id}"))?;
    let mutation =
        load_mutation(memory_root, run_id).unwrap_or_else(|_| synthetic_mutation(&entry));
    let replay = load_replay_info(memory_root, run_id)?;
    let report = build_report(&entry, &mutation, replay);
    persist_report(memory_root, &report)?;
    Ok(report)
}

pub fn print_last_report(memory_root: &str) -> Result<String, String> {
    let path = latest_report_path(memory_root)?
        .ok_or_else(|| "no russian reports available".to_string())?;
    fs::read_to_string(path).map_err(|error| format!("failed to read latest report: {error}"))
}

pub fn print_report(memory_root: &str, run_id: &str) -> Result<String, String> {
    let path = Path::new(memory_root)
        .join("reports")
        .join(format!("{run_id}.ru.md"));
    fs::read_to_string(path).map_err(|error| format!("failed to read report: {error}"))
}

pub fn load_report_json(memory_root: &str, run_id: &str) -> Result<EvolutionReport, String> {
    let path = Path::new(memory_root)
        .join("reports")
        .join(format!("{run_id}.report.json"));
    let contents =
        fs::read_to_string(path).map_err(|error| format!("failed to read report json: {error}"))?;
    serde_json::from_str(&contents).map_err(|error| format!("failed to parse report json: {error}"))
}

fn persist_report(memory_root: &str, report: &EvolutionReport) -> Result<(), String> {
    let dir = Path::new(memory_root).join("reports");
    fs::create_dir_all(&dir)
        .map_err(|error| format!("failed to create reports directory: {error}"))?;
    fs::write(
        dir.join(format!("{}.ru.md", report.run_id)),
        render_markdown(report),
    )
    .map_err(|error| format!("failed to write russian markdown report: {error}"))?;
    memory::write_json(dir.join(format!("{}.report.json", report.run_id)), report)
}

fn build_report(
    entry: &EvolutionLogEntry,
    mutation: &MutationContract,
    replay: ReplayInfo,
) -> EvolutionReport {
    EvolutionReport {
        run_id: entry.run_id.clone(),
        status: entry.status,
        goal_ru: goal_ru(entry),
        selected_plan_ru: selected_plan_ru(entry),
        mutation_ru: mutation_ru(entry, mutation),
        target_file: entry.target_file.clone(),
        mutation_kind: entry.mutation_kind.clone(),
        hypothesis_id: entry.hypothesis_id.clone(),
        source_patterns: entry.recombined_source_patterns.clone(),
        avoided_risks: entry.recombined_avoided_risks.clone(),
        recombination_reason_ru: entry.recombination_reason_ru.clone(),
        portfolio_reason_ru: entry.portfolio_reason_ru.clone(),
        diversity_bonus: entry.diversity_bonus,
        saturation_penalty: entry.saturation_penalty,
        repeated_target_penalty: entry.repeated_target_penalty,
        final_recombination_score: entry.final_recombination_score,
        sandbox_ru: sandbox_ru(entry),
        checks_ru: checks_ru(entry),
        score_ru: score_ru(entry),
        candidate_ru: candidate_ru(entry),
        replay_ru: replay.replay_ru,
        replay_status: replay.replay_status,
        replay_checked_at: replay.replay_checked_at,
        risk_ru: risk_ru(entry),
        next_step_ru: next_step_ru(entry),
    }
}

fn render_markdown(report: &EvolutionReport) -> String {
    let recombination_block = if report.source_patterns.is_empty()
        && report.avoided_risks.is_empty()
        && report.recombination_reason_ru.is_none()
        && report.portfolio_reason_ru.is_none()
    {
        String::new()
    } else {
        format!(
            "\n## Рекомбинация\nГипотеза: {}\nSource patterns: {}\nAvoided risks: {}\nПричина: {}\nPortfolio reason: {}\nDiversity bonus: {:.2}\nSaturation penalty: {:.2}\nRepeated target penalty: {:.2}\nFinal recombination score: {:.2}\n",
            report.hypothesis_id.as_deref().unwrap_or("нет"),
            if report.source_patterns.is_empty() {
                "(none)".to_string()
            } else {
                report.source_patterns.join(", ")
            },
            if report.avoided_risks.is_empty() {
                "(none)".to_string()
            } else {
                report.avoided_risks.join(", ")
            },
            report
                .recombination_reason_ru
                .as_deref()
                .unwrap_or("нет"),
            report.portfolio_reason_ru.as_deref().unwrap_or("нет"),
            report.diversity_bonus,
            report.saturation_penalty,
            report.repeated_target_penalty,
            report.final_recombination_score
        )
    };
    format!(
        "# Отчёт EVA\n\n## Цель\n{}\n\n## Гипотеза\n{}\n{}\n## Мутация\n{}\nФайл: {}\nТип: {}\n\n## Sandbox\n{}\n\n## Проверка\n{}\n\n## Решение\n{}\n\n## Повторное воспроизведение\nСтатус: {}\n{}\n{}\n\n## Риск\n{}\n\n## Следующий шаг\n{}\n",
        report.goal_ru,
        report.selected_plan_ru,
        recombination_block,
        report.mutation_ru,
        report.target_file,
        report.mutation_kind,
        report.sandbox_ru,
        report.checks_ru,
        report.candidate_ru,
        replay_status_ru(&report.replay_status),
        report.replay_ru,
        report
            .replay_checked_at
            .map(|value| format!("Проверено: {value}"))
            .unwrap_or_else(|| "Проверено: нет".to_string()),
        report.risk_ru,
        report.next_step_ru
    )
}

fn load_log_entry(memory_root: &str, run_id: &str) -> Result<Option<EvolutionLogEntry>, String> {
    let path = Path::new(memory_root).join("evolution.jsonl");
    if !path.exists() {
        return Ok(None);
    }
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read evolution log: {error}"))?;
    for line in contents.lines().rev() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<EvolutionLogEntry>(line) {
            if entry.run_id == run_id {
                return Ok(Some(entry));
            }
        }
    }
    Ok(None)
}

fn load_candidate_entry(
    memory_root: &str,
    run_id: &str,
) -> Result<Option<EvolutionLogEntry>, String> {
    let summary = match memory::load_candidate_summary(memory_root, run_id) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };
    Ok(Some(EvolutionLogEntry {
        run_id: summary.run_id.clone(),
        plan_id: None,
        hypothesis_id: None,
        objective: None,
        graph_evidence: Vec::new(),
        recombined_source_patterns: Vec::new(),
        recombined_avoided_risks: Vec::new(),
        recombination_reason_ru: None,
        portfolio_reason_ru: None,
        diversity_bonus: 0.0,
        saturation_penalty: 0.0,
        repeated_target_penalty: 0.0,
        final_recombination_score: 0.0,
        mutation_id: summary.mutation_id.clone(),
        mutation_digest: summary.mutation_digest.clone(),
        status: summary.status,
        target_file: summary.target_file.clone(),
        mutation_kind: summary.mutation_kind.clone(),
        risk: summary.risk,
        score: summary.score,
        useful_change: summary.useful_change,
        non_candidate_reason: summary.non_candidate_reason.clone(),
        duplicate_rejected: summary.duplicate_rejected,
        regression_penalty: summary.regression_penalty,
        success_bonus: summary.success_bonus,
        cargo_check_ok: summary.cargo_check_ok,
        cargo_test_ok: summary.cargo_test_ok,
        cargo_run_ok: summary.cargo_run_ok,
        retained_in_core: false,
        sandbox_destroyed: true,
        stdout_digest: summary.stdout_digest.clone(),
        stderr_digest: summary.stderr_digest.clone(),
        stderr_tail: summary.stderr_tail.clone(),
        timestamp_unix: summary.timestamp_unix,
    }))
}

fn load_mutation(memory_root: &str, run_id: &str) -> Result<MutationContract, String> {
    memory::load_candidate(memory_root, run_id)
}

fn synthetic_mutation(entry: &EvolutionLogEntry) -> MutationContract {
    MutationContract {
        id: entry.mutation_id.clone(),
        kind: mutation_kind_from_label(&entry.mutation_kind),
        target_file: entry.target_file.clone(),
        search: None,
        replace: None,
        append: None,
        reason: "reconstructed from evolution log".to_string(),
        expected_gain: 0.0,
        risk: entry.risk,
    }
}

fn load_replay_info(memory_root: &str, run_id: &str) -> Result<ReplayInfo, String> {
    let path = Path::new(memory_root)
        .join("replays")
        .join(format!("{run_id}.json"));
    if !path.exists() {
        return Ok(ReplayInfo {
            replay_ru: "Replay для этого запуска ещё не выполнялся.".to_string(),
            replay_status: "not_run".to_string(),
            replay_checked_at: None,
        });
    }
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read replay result: {error}"))?;
    let replay: ReplayResult = serde_json::from_str(&contents)
        .map_err(|error| format!("failed to parse replay result: {error}"))?;
    let passed = replay.matches_stored_summary
        && replay.cargo_check_ok
        && replay.cargo_test_ok
        && replay.cargo_run_ok
        && replay.replay_status != EvolutionStatus::Failed;
    Ok(ReplayInfo {
        replay_ru: if passed {
            "Replay пройден и синхронизирован с сохранённым summary.".to_string()
        } else {
            "Replay не пройден или не совпал с сохранённым summary.".to_string()
        },
        replay_status: if passed {
            "ok".to_string()
        } else {
            "failed".to_string()
        },
        replay_checked_at: Some(replay.timestamp_unix),
    })
}

fn goal_ru(entry: &EvolutionLogEntry) -> String {
    match entry.objective.as_deref() {
        Some("ImproveTests") => {
            "Ева проверила безопасное усиление слоя тестов через sandbox-мутацию.".to_string()
        }
        Some("ImproveReplayability") => {
            "Ева проверила безопасное улучшение replay/candidate логики через sandbox-мутацию."
                .to_string()
        }
        Some("ImproveGraphMemory") => {
            "Ева проверила безопасное улучшение graph/metrics слоя через sandbox-мутацию."
                .to_string()
        }
        _ => "Ева проверила безопасное улучшение проекта через sandbox-мутацию.".to_string(),
    }
}

fn selected_plan_ru(entry: &EvolutionLogEntry) -> String {
    if let Some(plan_id) = &entry.plan_id {
        format!(
            "Выбран план {} с целью {}.",
            plan_id,
            entry.objective.as_deref().unwrap_or("sandbox evolution")
        )
    } else {
        "Выполнен базовый безопасный цикл self-evolution без ручной promotion.".to_string()
    }
}

fn mutation_ru(entry: &EvolutionLogEntry, mutation: &MutationContract) -> String {
    format!(
        "Добавлена bounded-мутация {} по причине: {}.",
        entry.mutation_kind, mutation.reason
    )
}

fn sandbox_ru(entry: &EvolutionLogEntry) -> String {
    if entry.duplicate_rejected {
        "Мутация была отклонена до запуска sandbox, потому что совпала с ранее неудачной мутацией."
            .to_string()
    } else if entry.sandbox_destroyed {
        "Мутация была выполнена только в sandbox-копии проекта, после проверки sandbox удалён."
            .to_string()
    } else {
        "Мутация была выполнена только в sandbox-копии проекта.".to_string()
    }
}

fn checks_ru(entry: &EvolutionLogEntry) -> String {
    format!(
        "- cargo check: {}\n- cargo test: {}\n- cargo run -- --once: {}",
        status_ru(entry.cargo_check_ok),
        status_ru(entry.cargo_test_ok),
        status_ru(entry.cargo_run_ok)
    )
}

fn score_ru(entry: &EvolutionLogEntry) -> String {
    let useful = if entry.useful_change {
        "полезное"
    } else {
        "неполезное"
    };
    format!(
        "Итоговая оценка {:.1}. Изменение классифицировано как {}.",
        entry.score, useful
    )
}

fn candidate_ru(entry: &EvolutionLogEntry) -> String {
    match entry.status {
        EvolutionStatus::Candidate => {
            "Изменение признано полезным кандидатом. В core изменение не применялось.".to_string()
        }
        EvolutionStatus::Promoted => "Изменение прошло gate и было применено в core.".to_string(),
        EvolutionStatus::Passed => {
            format!(
                "Изменение прошло проверки, но не стало кандидатом. {}",
                score_ru(entry)
            )
        }
        EvolutionStatus::Failed => {
            "Изменение не прошло полный набор проверок и не было сохранено.".to_string()
        }
    }
}

fn risk_ru(entry: &EvolutionLogEntry) -> String {
    let level = if entry.risk <= 0.15 {
        "Низкий"
    } else if entry.risk <= 0.35 {
        "Средний"
    } else {
        "Повышенный"
    };
    format!(
        "{}. Риск мутации {:.2}, target={}, duplicate_rejected={}.",
        level, entry.risk, entry.target_file, entry.duplicate_rejected
    )
}

fn next_step_ru(entry: &EvolutionLogEntry) -> String {
    match entry.status {
        EvolutionStatus::Candidate => {
            format!("Следующий шаг: выполнить `cargo run -- --replay {}` и затем при необходимости `cargo run -- --promote {}`.", entry.run_id, entry.run_id)
        }
        EvolutionStatus::Promoted => {
            "Следующий шаг: зафиксировать изменение и наблюдать метрики следующих запусков."
                .to_string()
        }
        EvolutionStatus::Passed => {
            "Следующий шаг: усилить шаблон мутации или выбрать более полезный target.".to_string()
        }
        EvolutionStatus::Failed => {
            "Следующий шаг: уменьшить риск, изменить шаблон и повторить sandbox-проверку."
                .to_string()
        }
    }
}

fn latest_report_path(memory_root: &str) -> Result<Option<PathBuf>, String> {
    let dir = Path::new(memory_root).join("reports");
    if !dir.exists() {
        return Ok(None);
    }
    let mut files = fs::read_dir(&dir)
        .map_err(|error| format!("failed to read reports directory: {error}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".ru.md"))
        })
        .collect::<Vec<_>>();
    files.sort();
    Ok(files.pop())
}

fn mutation_kind_from_label(label: &str) -> MutationKind {
    match label {
        "appendcomment" => MutationKind::AppendComment,
        "replacetext" => MutationKind::ReplaceText,
        "parametertune" => MutationKind::ParameterTune,
        "addtestskeleton" => MutationKind::AddTestSkeleton,
        "addmetricfield" => MutationKind::AddMetricField,
        "addunittest" => MutationKind::AddUnitTest,
        "addreplayassertion" => MutationKind::AddReplayAssertion,
        "addlearningsummaryfield" => MutationKind::AddLearningSummaryField,
        "addmetricupdate" => MutationKind::AddMetricUpdate,
        _ => MutationKind::AppendComment,
    }
}

fn replay_status_ru(status: &str) -> &'static str {
    match status {
        "ok" => "пройден",
        "failed" => "не пройден",
        _ => "не выполнялся",
    }
}

fn status_ru(ok: bool) -> &'static str {
    if ok {
        "пройден"
    } else {
        "не пройден"
    }
}
