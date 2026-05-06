use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::contracts::{EvolutionLogEntry, EvolutionStatus};
use crate::evolution::{load_regressions, load_success_patterns};
use crate::{replay_candidate, run_planned_evolution_cycle};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvolutionBenchmark {
    pub benchmark_id: String,
    pub total_cycles: u64,
    pub passed_cycles: u64,
    pub failed_cycles: u64,
    pub useful_candidates: u64,
    pub cosmetic_runs: u64,
    pub duplicate_rejections: u64,
    pub replay_attempted: u64,
    pub replay_passed: u64,
    pub replay_failed: u64,
    pub regression_patterns_added: u64,
    pub success_patterns_added: u64,
    pub average_score: f32,
    pub candidate_rate: f32,
    pub replay_pass_rate: f32,
    pub forbidden_mutations: u64,
    pub sandbox_leaks: u64,
    pub started_at: u64,
    pub finished_at: u64,
}

pub fn run_planned_cycles(
    project_root: &str,
    memory_root: &str,
    cycles: usize,
) -> Result<Vec<String>, String> {
    let mut run_ids = Vec::new();
    for _ in 0..cycles {
        run_planned_evolution_cycle(project_root, memory_root)?;
        let last = latest_log_entry(memory_root)?
            .ok_or_else(|| "planned cycle completed without evolution log entry".to_string())?;
        run_ids.push(last.run_id);
    }
    Ok(run_ids)
}

pub fn run_benchmark(
    project_root: &str,
    memory_root: &str,
    cycles: usize,
) -> Result<EvolutionBenchmark, String> {
    let started_at = crate::evolution::memory::now_unix();
    let benchmark_id = format!("benchmark-{}-{cycles}", started_at);
    let before_regressions = load_regressions(memory_root)?.len() as u64;
    let before_successes = load_success_patterns(memory_root)?.len() as u64;

    let mut benchmark = EvolutionBenchmark {
        benchmark_id: benchmark_id.clone(),
        total_cycles: 0,
        passed_cycles: 0,
        failed_cycles: 0,
        useful_candidates: 0,
        cosmetic_runs: 0,
        duplicate_rejections: 0,
        replay_attempted: 0,
        replay_passed: 0,
        replay_failed: 0,
        regression_patterns_added: 0,
        success_patterns_added: 0,
        average_score: 0.0,
        candidate_rate: 0.0,
        replay_pass_rate: 0.0,
        forbidden_mutations: 0,
        sandbox_leaks: 0,
        started_at,
        finished_at: started_at,
    };

    let mut total_score = 0.0_f32;
    for _ in 0..cycles {
        let _ = run_planned_evolution_cycle(project_root, memory_root);
        let entry = latest_log_entry(memory_root)?
            .ok_or_else(|| "benchmark cycle completed without evolution log entry".to_string())?;
        benchmark.total_cycles += 1;
        total_score += entry.score;

        if entry.status == EvolutionStatus::Failed {
            benchmark.failed_cycles += 1;
        } else {
            benchmark.passed_cycles += 1;
        }
        if entry.useful_change && entry.status == EvolutionStatus::Candidate {
            benchmark.useful_candidates += 1;
        }
        if !entry.useful_change {
            benchmark.cosmetic_runs += 1;
        }
        if entry.duplicate_rejected {
            benchmark.duplicate_rejections += 1;
        }
        if is_forbidden_target(&entry.target_file) {
            benchmark.forbidden_mutations += 1;
        }

        let leaks = count_sandbox_leaks(project_root)?;
        benchmark.sandbox_leaks += leaks;

        if entry.useful_change && entry.status == EvolutionStatus::Candidate {
            benchmark.replay_attempted += 1;
            let replay_result = replay_candidate(project_root, memory_root, &entry.run_id);
            let replay = load_replay(memory_root, &entry.run_id).ok();
            if replay_result.is_ok() && replay.as_ref().is_some_and(replay_passed) {
                benchmark.replay_passed += 1;
            } else {
                benchmark.replay_failed += 1;
            }
        }
    }

    benchmark.finished_at = crate::evolution::memory::now_unix();
    benchmark.regression_patterns_added =
        load_regressions(memory_root)?.len() as u64 - before_regressions;
    benchmark.success_patterns_added =
        load_success_patterns(memory_root)?.len() as u64 - before_successes;
    if benchmark.total_cycles > 0 {
        benchmark.average_score = total_score / benchmark.total_cycles as f32;
        benchmark.candidate_rate =
            benchmark.useful_candidates as f32 / benchmark.total_cycles as f32;
    }
    if benchmark.replay_attempted > 0 {
        benchmark.replay_pass_rate =
            benchmark.replay_passed as f32 / benchmark.replay_attempted as f32;
    }
    benchmark.sandbox_leaks += count_sandbox_leaks(project_root)?;

    write_benchmark(memory_root, &benchmark)?;
    Ok(benchmark)
}

pub fn print_benchmark(benchmark: &EvolutionBenchmark) -> String {
    serde_json::to_string_pretty(benchmark).unwrap_or_else(|_| "{}".to_string())
}

fn write_benchmark(memory_root: &str, benchmark: &EvolutionBenchmark) -> Result<(), String> {
    let dir = Path::new(memory_root).join("benchmarks");
    fs::create_dir_all(&dir)
        .map_err(|error| format!("failed to create benchmark directory: {error}"))?;
    crate::evolution::memory::write_json(
        dir.join(format!("{}.json", benchmark.benchmark_id)),
        benchmark,
    )?;
    fs::write(
        dir.join(format!("{}.ru.md", benchmark.benchmark_id)),
        render_benchmark_markdown(benchmark),
    )
    .map_err(|error| format!("failed to write benchmark markdown: {error}"))
}

fn render_benchmark_markdown(benchmark: &EvolutionBenchmark) -> String {
    format!(
        "# Benchmark EVA\n\n## Цель проверки\nПроверить серию безопасных planned sandbox-циклов без auto-promotion.\n\n## Количество циклов\n{}\n\n## Полезные кандидаты\n{}\n\n## Косметические/неполезные мутации\n{}\n\n## Replay\nПопыток: {}\nПройдено: {}\nНе пройдено: {}\n\n## Регрессии и дубли\nНовых регрессий: {}\nДубликатов: {}\n\n## Sandbox cleanup\nОстаточных sandbox: {}\n\n## Средний score\n{:.2}\n\n## Вывод EVA\nCandidate rate: {:.2}\nReplay pass rate: {:.2}\nForbidden mutations: {}\n\n## Рекомендуемый следующий шаг\n{}",
        benchmark.total_cycles,
        benchmark.useful_candidates,
        benchmark.cosmetic_runs,
        benchmark.replay_attempted,
        benchmark.replay_passed,
        benchmark.replay_failed,
        benchmark.regression_patterns_added,
        benchmark.duplicate_rejections,
        benchmark.sandbox_leaks,
        benchmark.average_score,
        benchmark.candidate_rate,
        benchmark.replay_pass_rate,
        benchmark.forbidden_mutations,
        if benchmark.replay_passed > 0 {
            "Можно усиливать autonomy gate до следующего уровня после накопления достаточной серии запусков."
        } else {
            "Нужно накопить replay-подтверждённые полезные кандидаты."
        }
    )
}

fn latest_log_entry(memory_root: &str) -> Result<Option<EvolutionLogEntry>, String> {
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
            return Ok(Some(entry));
        }
    }
    Ok(None)
}

fn load_replay(memory_root: &str, run_id: &str) -> Result<crate::evolution::ReplayResult, String> {
    let path = Path::new(memory_root)
        .join("replays")
        .join(format!("{run_id}.json"));
    let contents =
        fs::read_to_string(path).map_err(|error| format!("failed to read replay file: {error}"))?;
    serde_json::from_str(&contents).map_err(|error| format!("failed to parse replay file: {error}"))
}

fn replay_passed(replay: &crate::evolution::ReplayResult) -> bool {
    replay.matches_stored_summary
        && replay.cargo_check_ok
        && replay.cargo_test_ok
        && replay.cargo_run_ok
        && replay.replay_status != EvolutionStatus::Failed
}

pub fn count_sandbox_leaks(project_root: &str) -> Result<u64, String> {
    let dir = Path::new(project_root).join("sandboxes");
    if !dir.exists() {
        return Ok(0);
    }
    Ok(fs::read_dir(&dir)
        .map_err(|error| format!("failed to read sandboxes: {error}"))?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name() != ".gitkeep")
        .count() as u64)
}

fn is_forbidden_target(target_file: &str) -> bool {
    target_file.starts_with("src/core/")
        || target_file == "src/main.rs"
        || target_file == "src/lib.rs"
        || target_file == "Cargo.toml"
        || target_file.ends_with("/Cargo.toml")
}
