use std::fs;
use std::path::{Path, PathBuf};

#[path = "evolution_test_support.rs"]
mod evolution_test_support;

use eva_runtime_with_task_validator::evolution::{
    compute_mutation_digest, load_stored_task_contract, record_dedup_entry,
};
use eva_runtime_with_task_validator::graph::{GraphEdge, GraphNode};
use eva_runtime_with_task_validator::{
    default_corpus_contract, generate_from_plan, ingest_corpus, list_suggested_tasks,
    load_corpus_summary, print_campaign_report, propose_mutation_plans_for_task,
    run_task_from_path, suggest_strategy_tasks, CampaignBlockerCount, CorpusIngestContract,
    DeniedMutationKind, EvolutionCampaign, EvolutionGraph, MutationKind, TaskContract,
};

#[test]
fn zero_candidate_campaign_writes_reason_and_blockers() {
    let root = temp_campaign_crate("phase55-zero-target");
    seed_campaign_memory(&root);
    seed_graph(&root, &[graph_file("src/reliability.rs")]);
    let task = safe_task("phase55_zero_target");
    let task = TaskContract {
        allowed_targets: vec!["src/evolution/*".to_string()],
        source_corpus_id: Some("corpus_demo".to_string()),
        ..task
    };
    let path = write_task(&root, &task);

    let campaign = run_task_from_path(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        path.to_str().unwrap(),
    )
    .expect("campaign");

    assert_eq!(
        campaign.zero_candidate_reason.as_deref(),
        Some("task_constraints_too_narrow")
    );
    assert!(!campaign.blocker_counts.is_empty());
    assert!(campaign.allowed_target_miss_count > 0);
    assert_eq!(campaign.useful_candidates, 0);

    let report =
        print_campaign_report(root.join("memory").to_str().unwrap(), &campaign.campaign_id)
            .expect("report");
    assert!(report.contains("## Диагностика результата"));
    assert!(report.contains("zero_candidate_reason=task_constraints_too_narrow"));

    let feedback_path = root
        .join("memory/tasks/feedback")
        .join(format!("{}.json", task.task_id));
    assert!(feedback_path.exists());
    let feedback_json = fs::read_to_string(feedback_path).expect("feedback");
    assert!(feedback_json.contains("recommended_adjustments"));
    let stored_task = fs::read_to_string(
        root.join("memory/tasks")
            .join(format!("{}.task.json", task.task_id)),
    )
    .expect("stored task");
    assert!(!stored_task.contains("recommended_adjustments"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn allowed_kind_and_below_score_counts_increment() {
    let root = temp_campaign_crate("phase55-kind-below");
    seed_campaign_memory(&root);
    seed_graph(&root, &[graph_file("src/reliability.rs")]);

    let kind_task = TaskContract {
        task_id: "phase55_kind_miss".to_string(),
        allowed_mutation_kinds: vec![MutationKind::AddMetricUpdate],
        ..safe_task("phase55_kind_miss")
    };
    let kind_path = write_task(&root, &kind_task);
    let kind_campaign = run_task_from_path(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        kind_path.to_str().unwrap(),
    )
    .expect("kind campaign");
    assert_eq!(
        kind_campaign.zero_candidate_reason.as_deref(),
        Some("task_constraints_too_narrow")
    );
    assert!(kind_campaign.allowed_kind_miss_count > 0);

    let below_task = TaskContract {
        task_id: "phase55_below_score".to_string(),
        min_score: 10.5,
        ..safe_task("phase55_below_score")
    };
    let below_path = write_task(&root, &below_task);
    let below_campaign = run_task_from_path(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        below_path.to_str().unwrap(),
    )
    .expect("below campaign");
    assert_eq!(
        below_campaign.zero_candidate_reason.as_deref(),
        Some("all_candidates_below_min_score")
    );
    assert!(below_campaign.below_score_count > 0);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn duplicate_rejection_count_increments_correctly() {
    let root = temp_campaign_crate("phase55-duplicate");
    seed_campaign_memory(&root);
    seed_graph(&root, &[graph_file("src/reliability.rs")]);
    let task = safe_task("phase55_duplicate");
    let path = write_task(&root, &task);
    let plans = propose_mutation_plans_for_task(root.join("memory").to_str().unwrap(), Some(&task))
        .expect("plans");
    let plan = plans.first().expect("first plan");
    let mutation = generate_from_plan(plan);
    let digest = compute_mutation_digest(&mutation);
    record_dedup_entry(
        root.join("memory").to_str().unwrap(),
        &digest,
        &mutation,
        2.0,
        false,
        "bad-run-1",
    )
    .expect("seed dedup");

    let campaign = run_task_from_path(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        path.to_str().unwrap(),
    )
    .expect("campaign");
    assert_eq!(
        campaign.zero_candidate_reason.as_deref(),
        Some("all_candidates_duplicate")
    );
    assert!(campaign.duplicate_rejection_count > 0);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn last_campaign_report_selects_newest_by_finished_at_and_campaign_report_loads_explicit() {
    let root = temp_campaign_crate("phase55-report-select");
    fs::create_dir_all(root.join("memory/campaigns")).expect("campaigns");
    let older = sample_campaign("campaign-old", "task_old", 10, 11);
    let newer = sample_campaign("campaign-new", "task_new", 10, 22);
    write_campaign_fixture(&root, &older, "old report");
    write_campaign_fixture(&root, &newer, "new report");

    let output = run_ok(&root, &["--last-campaign-report"]);
    assert!(output.contains("new report"));

    let explicit = run_ok(&root, &["--campaign-report", "campaign-old"]);
    assert!(explicit.contains("old report"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn missing_campaign_report_rebuilds_from_json() {
    let root = temp_campaign_crate("phase55-report-rebuild");
    seed_campaign_memory(&root);
    let task = safe_task("phase55_report_rebuild");
    fs::create_dir_all(root.join("memory/tasks")).expect("tasks");
    fs::write(
        root.join("memory/tasks")
            .join(format!("{}.task.json", task.task_id)),
        serde_json::to_string_pretty(&task).expect("task json"),
    )
    .expect("write task");
    fs::create_dir_all(root.join("memory/campaigns")).expect("campaigns");
    let campaign = sample_campaign("campaign-rebuild", &task.task_id, 15, 16);
    fs::write(
        root.join("memory/campaigns/campaign-rebuild.json"),
        serde_json::to_string_pretty(&campaign).expect("campaign json"),
    )
    .expect("write campaign");

    let report = run_ok(&root, &["--campaign-report", "campaign-rebuild"]);
    assert!(report.contains("## Диагностика результата"));
    assert!(root
        .join("memory/campaigns/campaign-rebuild.ru.md")
        .exists());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn corpus_latest_aliases_and_task_id_normalization_work() {
    let root = temp_campaign_crate("phase55-corpus-latest");
    let corpus_old = seed_corpus_repo(&root, "corpus_old_repo");
    let old_summary = ingest_corpus(
        root.join("memory").to_str().unwrap(),
        &CorpusIngestContract {
            corpus_id: "corpus_old".to_string(),
            ..default_corpus_contract(corpus_old.to_str().unwrap())
        },
    )
    .expect("old corpus");
    let corpus_new = seed_corpus_repo(&root, "corpus_new_repo");
    let new_summary = ingest_corpus(
        root.join("memory").to_str().unwrap(),
        &CorpusIngestContract {
            corpus_id: "corpus_cdb4ee2a".to_string(),
            ..default_corpus_contract(corpus_new.to_str().unwrap())
        },
    )
    .expect("new corpus");
    rewrite_corpus_generated_at(&root, &old_summary.corpus_id, 1);
    rewrite_corpus_generated_at(&root, &new_summary.corpus_id, 2);
    assert_ne!(old_summary.corpus_id, new_summary.corpus_id);

    let summary_output = run_ok(&root, &["--corpus-summary", "latest"]);
    assert!(summary_output.contains(&new_summary.corpus_id));

    let tasks_output = run_ok(&root, &["--suggest-strategy-tasks", "latest"]);
    assert!(tasks_output.contains("corpus_cdb4ee2a_test_expansion"));
    assert!(!tasks_output.contains("corpus_corpus_cdb4ee2a_test_expansion"));

    let tasks = suggest_strategy_tasks(
        root.join("memory").to_str().unwrap(),
        &new_summary.corpus_id,
    )
    .expect("tasks");
    assert!(tasks
        .iter()
        .any(|task| task.task_id == "corpus_cdb4ee2a_test_expansion"));

    let legacy_task = safe_task("corpus_corpus_legacy_test_expansion");
    fs::create_dir_all(root.join("memory/tasks")).expect("tasks dir");
    fs::write(
        root.join("memory/tasks")
            .join("corpus_corpus_legacy_test_expansion.task.json"),
        serde_json::to_string_pretty(&legacy_task).expect("legacy json"),
    )
    .expect("legacy write");
    fs::create_dir_all(root.join("memory/tasks/suggested")).expect("suggested dir");
    fs::write(
        root.join("memory/tasks/suggested")
            .join("corpus_corpus_cdb4ee2a_test_expansion.task.json"),
        serde_json::to_string_pretty(&legacy_task).expect("legacy suggested json"),
    )
    .expect("legacy suggested write");
    let listed = list_suggested_tasks(root.join("memory").to_str().unwrap()).expect("listed");
    assert!(listed
        .iter()
        .any(|task_id| task_id == "corpus_cdb4ee2a_test_expansion"));
    assert!(!listed
        .iter()
        .any(|task_id| task_id == "corpus_corpus_cdb4ee2a_test_expansion"));
    let loaded = load_stored_task_contract(
        root.join("memory").to_str().unwrap(),
        "corpus_corpus_legacy_test_expansion",
    )
    .expect("legacy load");
    assert_eq!(loaded.task_id, legacy_task.task_id);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn old_corpus_summary_without_generated_at_is_backward_compatible() {
    let root = temp_campaign_crate("phase55-old-summary");
    write_legacy_corpus_artifacts(&root, "corpus_cdb4ee2a");

    let summary = load_corpus_summary(root.join("memory").to_str().unwrap(), "corpus_cdb4ee2a")
        .expect("summary");
    assert_eq!(summary.corpus_id, "corpus_cdb4ee2a");
    assert_eq!(summary.generated_at, 0);

    let cli_summary = run_ok(&root, &["--corpus-summary", "corpus_cdb4ee2a"]);
    assert!(cli_summary.contains("\"corpus_id\": \"corpus_cdb4ee2a\""));
    let latest_summary = run_ok(&root, &["--corpus-summary", "latest"]);
    assert!(latest_summary.contains("\"corpus_id\": \"corpus_cdb4ee2a\""));

    let tasks = suggest_strategy_tasks(root.join("memory").to_str().unwrap(), "corpus_cdb4ee2a")
        .expect("tasks");
    assert!(tasks
        .iter()
        .any(|task| task.task_id == "corpus_cdb4ee2a_test_expansion"));
    let cli_tasks = run_ok(&root, &["--suggest-strategy-tasks", "corpus_cdb4ee2a"]);
    assert!(cli_tasks.contains("corpus_cdb4ee2a_test_expansion"));
    let cli_latest = run_ok(&root, &["--suggest-strategy-tasks", "latest"]);
    assert!(cli_latest.contains("corpus_cdb4ee2a_test_expansion"));

    fs::remove_dir_all(root).expect("cleanup");
}

fn temp_campaign_crate(name: &str) -> PathBuf {
    let root = evolution_test_support::unique_evolution_root(name);
    fs::create_dir_all(root.join("src")).expect("src");
    fs::create_dir_all(root.join("tests")).expect("tests");
    fs::create_dir_all(root.join("memory")).expect("memory");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"phase55_temp\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\ndoctest = false\n",
    )
    .expect("cargo");
    fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("main");
    fs::write(
        root.join("src/lib.rs"),
        "pub fn probe_status() -> bool { true }\n",
    )
    .expect("lib");
    root
}

fn seed_campaign_memory(root: &Path) {
    fs::write(root.join("memory/regressions.json"), "[]").expect("regressions");
    fs::write(root.join("memory/success_patterns.json"), "[]").expect("success");
    fs::create_dir_all(root.join("memory/replays")).expect("replays");
    let mut lines = Vec::new();
    for index in 0..12 {
        lines.push(format!(
            "{{\"run_id\":\"seed-{index}\",\"plan_id\":null,\"hypothesis_id\":null,\"objective\":\"ImproveTests\",\"graph_evidence\":[],\"recombined_source_patterns\":[],\"recombined_avoided_risks\":[],\"recombination_reason_ru\":null,\"portfolio_reason_ru\":null,\"selected_strategy\":null,\"policy_reason_ru\":null,\"mutation_class\":\"useful\",\"hygiene_warning_ru\":null,\"diversity_bonus\":0.0,\"saturation_penalty\":0.0,\"repeated_target_penalty\":0.0,\"final_recombination_score\":0.0,\"strategy_bonus\":0.0,\"strategy_saturation_penalty\":0.0,\"quality_bonus\":0.0,\"novelty_score\":0.0,\"useful_delta_score\":0.0,\"duplicate_suppression_score\":0.0,\"regression_avoidance_score\":0.0,\"coverage_proxy_score\":0.0,\"quality_score\":0.0,\"final_strategy_score\":0.0,\"mutation_id\":\"m-{index}\",\"mutation_digest\":\"d-{index}\",\"status\":\"candidate\",\"target_file\":\"tests/evolution_generated_tests.rs\",\"mutation_kind\":\"addunittest\",\"risk\":0.10,\"score\":8.50,\"useful_change\":true,\"non_candidate_reason\":null,\"duplicate_rejected\":false,\"regression_penalty\":0.0,\"success_bonus\":0.0,\"cargo_check_ok\":true,\"cargo_test_ok\":true,\"cargo_run_ok\":true,\"retained_in_core\":false,\"sandbox_destroyed\":true,\"stdout_digest\":\"\",\"stderr_digest\":\"\",\"stderr_tail\":\"\",\"timestamp_unix\":{}}}",
            index + 1
        ));
    }
    fs::write(root.join("memory/evolution.jsonl"), lines.join("\n") + "\n").expect("evolution");
    for index in 0..4 {
        fs::write(
            root.join("memory/replays")
                .join(format!("seed-{index}.json")),
            format!(
                "{{\"run_id\":\"seed-{index}\",\"replay_status\":\"passed\",\"score\":8.5,\"matches_stored_summary\":true,\"cargo_check_ok\":true,\"cargo_test_ok\":true,\"cargo_run_ok\":true,\"stdout_digest\":\"\",\"stderr_digest\":\"\",\"stderr_tail\":\"\",\"sandbox_destroyed\":true,\"timestamp_unix\":{}}}",
                index + 1
            ),
        )
        .expect("replay");
    }
}

fn seed_graph(root: &Path, files: &[GraphNode]) {
    let graph = EvolutionGraph {
        nodes: files.to_vec(),
        edges: files
            .iter()
            .map(|node| GraphEdge {
                from: "pattern:test".to_string(),
                to: node.id.clone(),
                relation: "supports".to_string(),
            })
            .collect(),
    };
    fs::write(
        root.join("memory/graph.json"),
        serde_json::to_string_pretty(&graph).expect("graph json"),
    )
    .expect("graph");
}

fn graph_file(path: &str) -> GraphNode {
    GraphNode {
        id: format!("file:{path}"),
        kind: "File".to_string(),
    }
}

fn safe_task(task_id: &str) -> TaskContract {
    TaskContract {
        task_id: task_id.to_string(),
        title_ru: "Phase55 task".to_string(),
        goal_ru: "Safe campaign for diagnostics".to_string(),
        allowed_targets: vec!["tests/*".to_string()],
        forbidden_targets: vec![
            "src/core/*".to_string(),
            "src/main.rs".to_string(),
            "src/lib.rs".to_string(),
            "Cargo.toml".to_string(),
        ],
        preferred_objectives: Vec::new(),
        allowed_mutation_kinds: vec![MutationKind::AddUnitTest, MutationKind::AddReplayAssertion],
        denied_mutation_kinds: vec![
            DeniedMutationKind::DeleteCode,
            DeniedMutationKind::RewriteFunction,
            DeniedMutationKind::FreeDiff,
            DeniedMutationKind::DependencyAdd,
        ],
        cycles: 1,
        require_replay: true,
        require_benchmark: false,
        require_russian_report: true,
        auto_promote: false,
        max_risk: 0.25,
        min_score: 7.0,
        source_corpus_id: None,
        created_at: 1,
    }
}

fn write_task(root: &Path, task: &TaskContract) -> PathBuf {
    let path = root.join(format!("{}.task.json", task.task_id));
    fs::write(
        &path,
        serde_json::to_string_pretty(task).expect("task json"),
    )
    .expect("task");
    path
}

fn sample_campaign(
    campaign_id: &str,
    task_id: &str,
    started_at: u64,
    finished_at: u64,
) -> EvolutionCampaign {
    EvolutionCampaign {
        campaign_id: campaign_id.to_string(),
        task_id: task_id.to_string(),
        source_corpus_id: None,
        source_task_id: task_id.to_string(),
        corpus_derived: false,
        total_cycles: 2,
        passed_cycles: 2,
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
        finished_at,
        blocker_counts: vec![CampaignBlockerCount {
            blocker: "no_valid_plan".to_string(),
            count: 1,
        }],
        candidate_run_ids: Vec::new(),
        zero_candidate_reason: Some("no_valid_plan".to_string()),
        rejected_plan_count: 1,
        duplicate_rejection_count: 0,
        below_score_count: 0,
        filtered_by_task_count: 0,
        no_valid_plan_count: 1,
        allowed_target_miss_count: 0,
        allowed_kind_miss_count: 0,
        already_promoted_count: 0,
        repeated_target_penalty_count: 0,
        generated_plan_count: 0,
        accepted_plan_count: 0,
        candidate_generated_count: 0,
        candidate_useful_count: 0,
        candidate_rejected_count: 0,
        candidate_rejected_below_min_score: 0,
        candidate_rejected_duplicate_payload: 0,
        candidate_rejected_failed_validator: 0,
        candidate_rejected_failed_replay: 0,
        candidate_rejected_not_useful: 0,
        candidate_rejected_already_promoted: 0,
        candidate_recovery_reason: Some("no_valid_plan".to_string()),
        recombination_fallback_attempted: false,
        recombination_fallback_used: false,
        recombination_candidates_seen: 0,
        recombination_accepted: 0,
        recombination_rejected_by_target: 0,
        recombination_rejected_by_kind: 0,
        recombination_rejected_by_risk: 0,
        recombination_rejected_by_forbidden_target: 0,
        recombination_rejected_by_class: 0,
        selected_hypothesis_id: None,
        selected_target: None,
        selected_kind: None,
        selected_risk: None,
        recombination_fallback_reason: Some("fixture".to_string()),
    }
}

fn write_campaign_fixture(root: &Path, campaign: &EvolutionCampaign, body: &str) {
    fs::write(
        root.join("memory/campaigns")
            .join(format!("{}.json", campaign.campaign_id)),
        serde_json::to_string_pretty(campaign).expect("campaign json"),
    )
    .expect("campaign json write");
    fs::write(
        root.join("memory/campaigns")
            .join(format!("{}.ru.md", campaign.campaign_id)),
        body,
    )
    .expect("campaign md write");
}

fn seed_corpus_repo(root: &Path, name: &str) -> PathBuf {
    let corpus = root.join(name);
    fs::create_dir_all(corpus.join("src")).expect("src");
    fs::create_dir_all(corpus.join("tests")).expect("tests");
    fs::create_dir_all(corpus.join("examples")).expect("examples");
    fs::write(
        corpus.join("src/lib.rs"),
        "pub fn validate_value(input: i32) -> Result<i32, ExampleError> { if input < 0 { return Err(ExampleError::Invalid); } Ok(input) }\npub enum ExampleError { Invalid }\nmod reporting;\n",
    )
    .expect("lib");
    fs::write(
        corpus.join("src/reporting.rs"),
        "pub fn write_report() { println!(\"report\"); }\n",
    )
    .expect("report");
    fs::write(
        corpus.join("tests/basic.rs"),
        "#[test]\nfn detects_assertions() { assert_eq!(2 + 2, 4); }\n",
    )
    .expect("test");
    fs::write(
        corpus.join("examples/cli.rs"),
        "fn cli() { let _ = clap::Command::new(\"demo\").subcommand(clap::Command::new(\"run\")); }\n",
    )
    .expect("cli");
    fs::write(corpus.join("Cargo.toml"), "[package]\nname=\"corpus\"\n").expect("toml");
    corpus
}

fn rewrite_corpus_generated_at(root: &Path, corpus_id: &str, generated_at: u64) {
    let path = root
        .join("memory/corpus")
        .join(format!("{}.summary.json", corpus_id));
    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("summary")).expect("json");
    value["generated_at"] = serde_json::Value::from(generated_at);
    fs::write(&path, serde_json::to_string_pretty(&value).expect("value")).expect("write summary");
}

fn write_legacy_corpus_artifacts(root: &Path, corpus_id: &str) {
    fs::create_dir_all(root.join("memory/corpus")).expect("corpus dir");
    fs::write(
        root.join("memory/corpus")
            .join(format!("{corpus_id}.summary.json")),
        format!(
            "{{\"corpus_id\":\"{corpus_id}\",\"root_path\":\"/tmp/local-corpus\",\"scanned_files\":4,\"skipped_files\":0,\"file_count\":4,\"rust_file_count\":3,\"test_file_count\":1,\"function_count\":3,\"test_function_count\":1,\"result_returning_functions\":1,\"error_enum_count\":1,\"validation_function_count\":1,\"cli_parser_mentions\":1,\"reporting_mentions\":1,\"module_names\":[\"reporting\"],\"suggested_strategies\":[\"TestExpansion\",\"ValidationHardening\",\"MetricsReporting\",\"RegressionAvoidance\"],\"safety_notes\":[\"read-only local ingestion only\"]}}"
        ),
    )
    .expect("legacy summary");
    fs::write(
        root.join("memory/corpus")
            .join(format!("{corpus_id}.patterns.json")),
        format!(
            "{{\"corpus_id\":\"{corpus_id}\",\"detected_patterns\":[\"test_assertion_pattern\",\"validation_guard_pattern\",\"report_writer_pattern\",\"result_error_pattern\"],\"symbolic_labels\":[\"assertion_label\"]}}"
        ),
    )
    .expect("legacy patterns");
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
