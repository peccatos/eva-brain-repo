use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "evolution_test_support.rs"]
mod evolution_test_support;

use eva_runtime_with_task_validator::{
    adjust_task_from_campaign, list_adjusted_tasks, print_last_task_adjustment, DeniedMutationKind,
    MutationKind, MutationObjective, TaskContract,
};

#[test]
fn zero_yield_campaign_creates_adjusted_task() {
    let root = temp_runtime_root("phase56-adjust");
    let task = narrow_task("corpus_phase56_test_expansion", 2, 7.0);
    seed_task(&root, &task);
    seed_campaign(
        &root,
        "campaign-adjust",
        &task.task_id,
        "task_constraints_too_narrow",
        16,
        0,
    );
    seed_feedback(&root, &task.task_id, "campaign-adjust");

    let adjustment =
        adjust_task_from_campaign(root.join("memory").to_str().unwrap(), "campaign-adjust")
            .expect("adjustment");

    assert_eq!(adjustment.source_task_id, task.task_id);
    assert!(Path::new(&adjustment.adjusted_task_path).exists());
    assert!(root
        .join("memory/tasks/adjusted")
        .join(format!("{}.adjustment.json", task.task_id))
        .exists());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn adjusted_task_keeps_safety_constraints() {
    let root = temp_runtime_root("phase56-safety");
    let task = narrow_task("corpus_phase56_safety", 2, 7.0);
    let original_json = seed_task(&root, &task);
    seed_campaign(
        &root,
        "campaign-safety",
        &task.task_id,
        "task_constraints_too_narrow",
        8,
        0,
    );

    let adjustment =
        adjust_task_from_campaign(root.join("memory").to_str().unwrap(), "campaign-safety")
            .expect("adjustment");
    let adjusted: TaskContract = serde_json::from_str(
        &fs::read_to_string(&adjustment.adjusted_task_path).expect("adjusted task"),
    )
    .expect("parse adjusted task");

    assert!(!adjusted.auto_promote);
    assert!(adjusted.require_russian_report);
    assert!(adjusted.require_replay);
    assert!(adjusted.max_risk <= 0.25);
    assert!(adjusted
        .forbidden_targets
        .contains(&"src/core/*".to_string()));
    assert!(adjusted
        .forbidden_targets
        .contains(&"src/main.rs".to_string()));
    assert!(adjusted
        .forbidden_targets
        .contains(&"src/lib.rs".to_string()));
    assert!(adjusted
        .forbidden_targets
        .contains(&"Cargo.toml".to_string()));
    assert!(adjusted.allowed_mutation_kinds.iter().all(|kind| matches!(
        kind,
        MutationKind::AddUnitTest
            | MutationKind::AddReplayAssertion
            | MutationKind::AddLearningSummaryField
            | MutationKind::AddMetricUpdate
    )));
    let original_after = fs::read_to_string(
        root.join("memory/tasks")
            .join(format!("{}.task.json", task.task_id)),
    )
    .expect("original task");
    assert_eq!(original_json, original_after);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn task_constraints_too_narrow_broadens_targets_safely() {
    let root = temp_runtime_root("phase56-broaden");
    let task = narrow_task("corpus_phase56_broaden", 2, 7.0);
    seed_task(&root, &task);
    seed_campaign(
        &root,
        "campaign-broaden",
        &task.task_id,
        "task_constraints_too_narrow",
        10,
        0,
    );

    let adjustment =
        adjust_task_from_campaign(root.join("memory").to_str().unwrap(), "campaign-broaden")
            .expect("adjustment");
    let adjusted = load_adjusted_task(&adjustment.adjusted_task_path);

    assert!(adjusted.allowed_targets.contains(&"tests/*".to_string()));
    assert!(adjusted
        .allowed_targets
        .contains(&"src/evolution/*".to_string()));
    assert!(adjusted
        .allowed_targets
        .contains(&"src/promotion/*".to_string()));
    assert!(adjusted
        .allowed_targets
        .contains(&"src/sandbox/*".to_string()));
    assert!(adjusted
        .allowed_targets
        .contains(&"src/runtime/*".to_string()));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn all_candidates_below_min_score_reduces_min_score_but_not_below_five() {
    let root = temp_runtime_root("phase56-min-score");
    let task = narrow_task("corpus_phase56_min_score", 2, 5.2);
    seed_task(&root, &task);
    seed_campaign(
        &root,
        "campaign-min-score",
        &task.task_id,
        "all_candidates_below_min_score",
        6,
        0,
    );

    let adjustment =
        adjust_task_from_campaign(root.join("memory").to_str().unwrap(), "campaign-min-score")
            .expect("adjustment");
    let adjusted = load_adjusted_task(&adjustment.adjusted_task_path);

    assert!(adjusted.min_score >= 5.0);
    assert_eq!(adjusted.min_score, 5.0);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn cycles_capped_at_five() {
    let root = temp_runtime_root("phase56-cycles");
    let task = narrow_task("corpus_phase56_cycles", 4, 7.0);
    seed_task(&root, &task);
    seed_campaign(
        &root,
        "campaign-cycles",
        &task.task_id,
        "task_constraints_too_narrow",
        12,
        0,
    );

    let adjustment =
        adjust_task_from_campaign(root.join("memory").to_str().unwrap(), "campaign-cycles")
            .expect("adjustment");
    let adjusted = load_adjusted_task(&adjustment.adjusted_task_path);
    assert_eq!(adjusted.cycles, 5);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn last_task_adjustment_prints_newest_adjustment() {
    let root = temp_runtime_root("phase56-last");
    let task_a = narrow_task("corpus_phase56_a", 2, 7.0);
    seed_task(&root, &task_a);
    seed_campaign(
        &root,
        "campaign-a",
        &task_a.task_id,
        "task_constraints_too_narrow",
        8,
        0,
    );
    let task_b = narrow_task("corpus_phase56_b", 2, 7.0);
    seed_task(&root, &task_b);
    seed_campaign(
        &root,
        "campaign-b",
        &task_b.task_id,
        "allowed_kinds_filtered_all",
        8,
        0,
    );

    adjust_task_from_campaign(root.join("memory").to_str().unwrap(), "campaign-a").expect("a");
    std::thread::sleep(std::time::Duration::from_millis(5));
    adjust_task_from_campaign(root.join("memory").to_str().unwrap(), "campaign-b").expect("b");

    let report = print_last_task_adjustment(root.join("memory").to_str().unwrap()).expect("report");
    assert!(report.contains("campaign-b"));

    let cli_report = run_ok(&root, &["--last-task-adjustment"]);
    assert!(cli_report.contains("campaign-b"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn list_adjusted_tasks_prints_adjusted_task_ids() {
    let root = temp_runtime_root("phase56-list");
    let task = narrow_task("corpus_phase56_list", 2, 7.0);
    seed_task(&root, &task);
    seed_campaign(
        &root,
        "campaign-list",
        &task.task_id,
        "task_constraints_too_narrow",
        8,
        0,
    );
    adjust_task_from_campaign(root.join("memory").to_str().unwrap(), "campaign-list")
        .expect("adjustment");

    let ids = list_adjusted_tasks(root.join("memory").to_str().unwrap()).expect("ids");
    assert!(ids.contains(&task.task_id));
    let cli = run_ok(&root, &["--list-adjusted-tasks"]);
    assert!(cli.contains(&task.task_id));

    fs::remove_dir_all(root).expect("cleanup");
}

fn temp_runtime_root(name: &str) -> PathBuf {
    let root = evolution_test_support::unique_evolution_root(name);
    fs::create_dir_all(root.join("src")).expect("src");
    fs::create_dir_all(root.join("memory/tasks")).expect("tasks");
    fs::create_dir_all(root.join("memory/campaigns")).expect("campaigns");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"phase56_temp\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("cargo");
    fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("main");
    root
}

fn narrow_task(task_id: &str, cycles: usize, min_score: f32) -> TaskContract {
    TaskContract {
        task_id: task_id.to_string(),
        title_ru: "Narrow task".to_string(),
        goal_ru: "Zero yield test task".to_string(),
        allowed_targets: vec!["tests/*".to_string()],
        forbidden_targets: vec![
            "src/core/*".to_string(),
            "src/main.rs".to_string(),
            "src/lib.rs".to_string(),
            "Cargo.toml".to_string(),
        ],
        preferred_objectives: vec![MutationObjective::ImproveTests],
        allowed_mutation_kinds: vec![MutationKind::AddUnitTest],
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
        max_risk: 0.2,
        min_score,
        source_corpus_id: Some("corpus_phase56".to_string()),
        created_at: 1,
    }
}

fn seed_task(root: &Path, task: &TaskContract) -> String {
    let path = root
        .join("memory/tasks")
        .join(format!("{}.task.json", task.task_id));
    let json = serde_json::to_string_pretty(task).expect("task json");
    fs::write(path, &json).expect("write task");
    json
}

fn seed_campaign(
    root: &Path,
    campaign_id: &str,
    task_id: &str,
    zero_candidate_reason: &str,
    filtered_by_task_count: usize,
    useful_candidates: u64,
) {
    let campaign = serde_json::json!({
        "campaign_id": campaign_id,
        "task_id": task_id,
        "source_corpus_id": "corpus_phase56",
        "source_task_id": task_id,
        "corpus_derived": true,
        "total_cycles": 2,
        "passed_cycles": 2,
        "failed_cycles": 0,
        "useful_candidates": useful_candidates,
        "replay_attempted": 0,
        "replay_passed": 0,
        "replay_failed": 0,
        "duplicate_rejections": 0,
        "regression_patterns_added": 0,
        "success_patterns_added": 0,
        "promotion_ready_candidates": 0,
        "promoted_candidates": 0,
        "forbidden_mutations": 0,
        "sandbox_leaks": 0,
        "average_score": 0.0,
        "started_at": 10,
        "finished_at": 20,
        "blocker_counts": [{"blocker": zero_candidate_reason, "count": 1}],
        "candidate_run_ids": [],
        "zero_candidate_reason": zero_candidate_reason,
        "rejected_plan_count": filtered_by_task_count,
        "duplicate_rejection_count": 0,
        "below_score_count": usize::from(zero_candidate_reason == "all_candidates_below_min_score"),
        "filtered_by_task_count": filtered_by_task_count,
        "no_valid_plan_count": 0,
        "allowed_target_miss_count": usize::from(zero_candidate_reason == "allowed_targets_filtered_all") * filtered_by_task_count,
        "allowed_kind_miss_count": usize::from(zero_candidate_reason == "allowed_kinds_filtered_all") * filtered_by_task_count,
        "already_promoted_count": usize::from(zero_candidate_reason == "all_candidates_already_promoted"),
        "repeated_target_penalty_count": 0,
        "generated_plan_count": filtered_by_task_count,
        "accepted_plan_count": 0
    });
    fs::write(
        root.join("memory/campaigns")
            .join(format!("{campaign_id}.json")),
        serde_json::to_string_pretty(&campaign).expect("campaign json"),
    )
    .expect("write campaign");
}

fn seed_feedback(root: &Path, task_id: &str, campaign_id: &str) {
    fs::create_dir_all(root.join("memory/tasks/feedback")).expect("feedback dir");
    let feedback = serde_json::json!({
        "task_id": task_id,
        "source_corpus_id": "corpus_phase56",
        "last_campaign_id": campaign_id,
        "zero_candidate_reason": "task_constraints_too_narrow",
        "recommended_adjustments": ["loosen allowed_targets", "add another allowed mutation kind"],
        "created_at": 1
    });
    fs::write(
        root.join("memory/tasks/feedback")
            .join(format!("{task_id}.json")),
        serde_json::to_string_pretty(&feedback).expect("feedback json"),
    )
    .expect("write feedback");
}

fn load_adjusted_task(path: &str) -> TaskContract {
    serde_json::from_str(&fs::read_to_string(path).expect("adjusted")).expect("parse adjusted task")
}

fn run_ok(root: &Path, args: &[&str]) -> String {
    let output = evolution_test_support::eva_command(root)
        .args(args)
        .output()
        .expect("run");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("stdout")
}
