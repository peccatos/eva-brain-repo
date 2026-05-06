use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::contracts::{EvolutionLogEntry, EvolutionStatus, TaskContract};
use crate::evolution::autonomy::autonomy_status;
use crate::evolution::benchmark::count_sandbox_leaks;
use crate::evolution::memory;
use crate::evolution::task_validator::{
    load_stored_task_contract, load_task_contract, store_task_contract, validate_task_contract,
};
use crate::promotion::review::review_candidate;
use crate::runtime::run_planned_evolution_cycle_for_task;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvolutionCampaign {
    pub campaign_id: String,
    pub task_id: String,
    pub total_cycles: u64,
    pub passed_cycles: u64,
    pub failed_cycles: u64,
    pub useful_candidates: u64,
    pub replay_attempted: u64,
    pub replay_passed: u64,
    pub replay_failed: u64,
    pub duplicate_rejections: u64,
    pub regression_patterns_added: u64,
    pub success_patterns_added: u64,
    pub promotion_ready_candidates: u64,
    pub promoted_candidates: u64,
    pub forbidden_mutations: u64,
    pub sandbox_leaks: u64,
    pub average_score: f32,
    pub started_at: u64,
    pub finished_at: u64,
    pub blocker_counts: Vec<CampaignBlockerCount>,
    pub candidate_run_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CampaignBlockerCount {
    pub blocker: String,
    pub count: u64,
}

pub fn run_task_from_path(
    project_root: &str,
    memory_root: &str,
    task_path: &str,
) -> Result<EvolutionCampaign, String> {
    let task = load_task_contract(Path::new(task_path))?;
    validate_task_contract(&task)?;
    store_task_contract(memory_root, &task)?;
    run_campaign(project_root, memory_root, &task)
}

pub fn run_stored_campaign(
    project_root: &str,
    memory_root: &str,
    task_id: &str,
) -> Result<EvolutionCampaign, String> {
    let task = load_stored_task_contract(memory_root, task_id)?;
    validate_task_contract(&task)?;
    run_campaign(project_root, memory_root, &task)
}

pub fn print_last_campaign_report(memory_root: &str) -> Result<String, String> {
    let dir = Path::new(memory_root).join("campaigns");
    let path =
        latest_markdown_path(&dir)?.ok_or_else(|| "no campaign reports available".to_string())?;
    fs::read_to_string(path).map_err(|error| format!("failed to read campaign report: {error}"))
}

pub fn print_campaign(campaign: &EvolutionCampaign) -> String {
    serde_json::to_string_pretty(campaign).unwrap_or_else(|_| "{}".to_string())
}

fn run_campaign(
    project_root: &str,
    memory_root: &str,
    task: &TaskContract,
) -> Result<EvolutionCampaign, String> {
    validate_task_contract(task)?;
    let autonomy = autonomy_status(project_root, memory_root)?;
    if autonomy.current_level < 3 || !autonomy.campaign_mode_allowed {
        return Err("campaign mode is blocked by autonomy gate".to_string());
    }
    if task.cycles > autonomy.max_campaign_cycles {
        return Err(format!(
            "campaign cycles exceed autonomy limit: {} > {}",
            task.cycles, autonomy.max_campaign_cycles
        ));
    }
    if task.require_benchmark && !has_benchmark_history(memory_root)? {
        return Err("task requires benchmark history before campaign run".to_string());
    }

    let started_at = memory::now_unix();
    let campaign_id = format!("campaign-{}-{}", task.task_id, started_at);
    let before_regressions = crate::evolution::load_regressions(memory_root)?.len() as u64;
    let before_successes = crate::evolution::load_success_patterns(memory_root)?.len() as u64;
    let before_promotions = count_promotions(memory_root)? as u64;
    let mut campaign = EvolutionCampaign {
        campaign_id: campaign_id.clone(),
        task_id: task.task_id.clone(),
        total_cycles: 0,
        passed_cycles: 0,
        failed_cycles: 0,
        useful_candidates: 0,
        replay_attempted: 0,
        replay_passed: 0,
        replay_failed: 0,
        duplicate_rejections: 0,
        regression_patterns_added: 0,
        success_patterns_added: 0,
        promotion_ready_candidates: 0,
        promoted_candidates: 0,
        forbidden_mutations: 0,
        sandbox_leaks: 0,
        average_score: 0.0,
        started_at,
        finished_at: started_at,
        blocker_counts: Vec::new(),
        candidate_run_ids: Vec::new(),
    };
    let mut blocker_counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut total_score = 0.0_f32;

    for _ in 0..task.cycles {
        let _ = run_planned_evolution_cycle_for_task(project_root, memory_root, Some(task));
        let entry = latest_log_entry(memory_root)?
            .ok_or_else(|| "campaign cycle completed without evolution log entry".to_string())?;
        campaign.total_cycles += 1;
        total_score += entry.score;

        if entry.status == EvolutionStatus::Failed {
            campaign.failed_cycles += 1;
        } else {
            campaign.passed_cycles += 1;
        }
        if entry.duplicate_rejected {
            campaign.duplicate_rejections += 1;
        }
        if is_forbidden_target(&entry.target_file) {
            campaign.forbidden_mutations += 1;
        }
        campaign.sandbox_leaks += count_sandbox_leaks(project_root)?;

        if entry.useful_change && entry.status == EvolutionStatus::Candidate {
            campaign.useful_candidates += 1;
            campaign.candidate_run_ids.push(entry.run_id.clone());
            if task.require_replay {
                campaign.replay_attempted += 1;
                let replay_result =
                    crate::promotion::replay_candidate(project_root, memory_root, &entry.run_id);
                let review = review_candidate(project_root, memory_root, &entry.run_id)?;
                if replay_result.is_ok() && review.replay_status == "ok" {
                    campaign.replay_passed += 1;
                } else {
                    campaign.replay_failed += 1;
                }
                if review.promotion_allowed {
                    campaign.promotion_ready_candidates += 1;
                }
                for blocker in &review.promotion_blockers {
                    *blocker_counts.entry(blocker.clone()).or_insert(0) += 1;
                }
            } else {
                let review = review_candidate(project_root, memory_root, &entry.run_id)?;
                if review.promotion_allowed {
                    campaign.promotion_ready_candidates += 1;
                }
                for blocker in &review.promotion_blockers {
                    *blocker_counts.entry(blocker.clone()).or_insert(0) += 1;
                }
            }
        }
    }

    campaign.finished_at = memory::now_unix();
    campaign.regression_patterns_added =
        crate::evolution::load_regressions(memory_root)?.len() as u64 - before_regressions;
    campaign.success_patterns_added =
        crate::evolution::load_success_patterns(memory_root)?.len() as u64 - before_successes;
    campaign.promoted_candidates = count_promotions(memory_root)? as u64 - before_promotions;
    campaign.sandbox_leaks += count_sandbox_leaks(project_root)?;
    campaign.average_score = if campaign.total_cycles == 0 {
        0.0
    } else {
        total_score / campaign.total_cycles as f32
    };
    campaign.blocker_counts = blocker_counts
        .into_iter()
        .map(|(blocker, count)| CampaignBlockerCount { blocker, count })
        .collect();

    write_campaign(memory_root, task, &campaign)?;
    Ok(campaign)
}

fn write_campaign(
    memory_root: &str,
    task: &TaskContract,
    campaign: &EvolutionCampaign,
) -> Result<(), String> {
    let dir = Path::new(memory_root).join("campaigns");
    fs::create_dir_all(&dir)
        .map_err(|error| format!("failed to create campaigns directory: {error}"))?;
    memory::write_json(dir.join(format!("{}.json", campaign.campaign_id)), campaign)?;
    fs::write(
        dir.join(format!("{}.ru.md", campaign.campaign_id)),
        render_campaign_markdown(task, campaign),
    )
    .map_err(|error| format!("failed to write campaign markdown: {error}"))
}

fn render_campaign_markdown(task: &TaskContract, campaign: &EvolutionCampaign) -> String {
    let blockers = if campaign.blocker_counts.is_empty() {
        "Нет явных blocker reason по кандидатам.".to_string()
    } else {
        campaign
            .blocker_counts
            .iter()
            .map(|item| format!("- {}: {}", item.blocker, item.count))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let ready = if campaign.candidate_run_ids.is_empty() {
        "Нет кандидатов для ручного promotion-review.".to_string()
    } else {
        campaign
            .candidate_run_ids
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "# Campaign EVA\n\n## Цель задачи\n{}\n\n## Ограничения\ncycles={} max_risk={:.2} require_replay={} auto_promote={}\nallowed_targets={:?}\nallowed_kinds={:?}\n\n## Количество циклов\n{}\n\n## Найденные кандидаты\nПолезных кандидатов: {}\nГотовых к promotion-review: {}\n\n## Replay\nПопыток: {}\nПройдено: {}\nНе пройдено: {}\n\n## Причины отказа по кандидатам\n{}\n\n## Готовые к promotion кандидаты\n{}\n\n## Риски\nforbidden_mutations={} sandbox_leaks={} duplicate_rejections={}\n\n## Итог EVA\nЕва выполнила {} sandbox-циклов. Найдено {} полезных кандидата(ов), {} прошли replay. Promotion автоматически не выполнялся.\n\n## Рекомендация следующего шага\n{}",
        task.goal_ru,
        task.cycles,
        task.max_risk,
        task.require_replay,
        task.auto_promote,
        task.allowed_targets,
        task.allowed_mutation_kinds,
        campaign.total_cycles,
        campaign.useful_candidates,
        campaign.promotion_ready_candidates,
        campaign.replay_attempted,
        campaign.replay_passed,
        campaign.replay_failed,
        blockers,
        ready,
        campaign.forbidden_mutations,
        campaign.sandbox_leaks,
        campaign.duplicate_rejections,
        campaign.total_cycles,
        campaign.useful_candidates,
        campaign.replay_passed,
        if campaign.promotion_ready_candidates > 0 {
            "Рекомендуется вручную рассмотреть replay-подтверждённые кандидаты через --review-candidate."
        } else {
            "Нужно накопить replay-подтверждённые полезные кандидаты без критических blockers."
        }
    )
}

fn has_benchmark_history(memory_root: &str) -> Result<bool, String> {
    let dir = Path::new(memory_root).join("benchmarks");
    if !dir.exists() {
        return Ok(false);
    }
    Ok(fs::read_dir(dir)
        .map_err(|error| format!("failed to read benchmark directory: {error}"))?
        .filter_map(Result::ok)
        .any(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "json")
        }))
}

fn latest_markdown_path(dir: &Path) -> Result<Option<PathBuf>, String> {
    if !dir.exists() {
        return Ok(None);
    }
    let mut candidates = fs::read_dir(dir)
        .map_err(|error| format!("failed to read campaign directory: {error}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
        .collect::<Vec<_>>();
    candidates.sort();
    Ok(candidates.pop())
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

fn count_promotions(memory_root: &str) -> Result<usize, String> {
    let path = Path::new(memory_root).join("evolution.jsonl");
    if !path.exists() {
        return Ok(0);
    }
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read evolution log: {error}"))?;
    Ok(contents
        .lines()
        .filter_map(|line| serde_json::from_str::<EvolutionLogEntry>(line).ok())
        .filter(|entry| entry.retained_in_core)
        .count())
}

fn is_forbidden_target(target_file: &str) -> bool {
    target_file.starts_with("src/core/")
        || target_file == "src/main.rs"
        || target_file == "src/lib.rs"
        || target_file == "Cargo.toml"
        || target_file.ends_with("/Cargo.toml")
}
