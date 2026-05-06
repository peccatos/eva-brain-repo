use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use eva_runtime_with_task_validator::load_report_json;

#[test]
fn replay_updates_russian_report() {
    let root = temp_crate("phase46r-replay-report");
    seed_graph(&root);
    run_ok(&root, &["--evolve-planned"]);
    let run_id = latest_candidate_run_id(&root);

    let before = load_report_json(root.join("memory").to_str().unwrap(), &run_id)
        .expect("report before replay");
    assert_eq!(before.replay_status, "not_run");

    run_ok(&root, &["--replay", &run_id]);

    let after = load_report_json(root.join("memory").to_str().unwrap(), &run_id)
        .expect("report after replay");
    assert_eq!(after.replay_status, "ok");
    assert!(after.replay_checked_at.is_some());
    let markdown = fs::read_to_string(root.join("memory/reports").join(format!("{run_id}.ru.md")))
        .expect("read markdown report");
    assert!(markdown.contains("Статус: пройден"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn report_refresh_updates_old_report() {
    let root = temp_crate("phase46r-report-refresh");
    seed_graph(&root);
    run_ok(&root, &["--evolve-planned"]);
    let run_id = latest_candidate_run_id(&root);
    run_ok(&root, &["--replay", &run_id]);

    fs::write(
        root.join("memory/reports").join(format!("{run_id}.ru.md")),
        "# stale\nReplay: не выполнялся\n",
    )
    .expect("write stale markdown");
    fs::write(
        root.join("memory/reports")
            .join(format!("{run_id}.report.json")),
        r#"{"run_id":"stale","replay_status":"not_run"}"#,
    )
    .expect("write stale json");

    let output = run_ok(&root, &["--report-refresh", &run_id]);
    assert!(output.contains("\"replay_status\": \"ok\""));
    let refreshed = fs::read_to_string(root.join("memory/reports").join(format!("{run_id}.ru.md")))
        .expect("read refreshed markdown");
    assert!(refreshed.contains("Статус: пройден"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn evolve_planned_n_runs_n_cycles() {
    let root = temp_crate("phase46r-planned-n");
    seed_graph(&root);
    let output = run_ok(&root, &["--evolve-planned-n", "3"]);
    let run_ids: Vec<String> = serde_json::from_str(&output).expect("parse run ids");
    assert_eq!(run_ids.len(), 3);
    let reports = report_count(&root);
    assert!(reports >= 3);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn benchmark_creates_reports_and_counts_useful_candidates_with_no_leaks() {
    let root = temp_crate("phase46r-benchmark");
    seed_graph(&root);
    let output = run_ok(&root, &["--evolution-benchmark", "2"]);
    let benchmark: serde_json::Value = serde_json::from_str(&output).expect("parse benchmark");
    let benchmark_id = benchmark["benchmark_id"].as_str().expect("benchmark id");
    assert_eq!(benchmark["total_cycles"].as_u64(), Some(2));
    assert!(benchmark["useful_candidates"].as_u64().unwrap_or(0) > 0);
    assert_eq!(benchmark["sandbox_leaks"].as_u64(), Some(0));
    assert!(root
        .join("memory/benchmarks")
        .join(format!("{benchmark_id}.json"))
        .exists());
    assert!(root
        .join("memory/benchmarks")
        .join(format!("{benchmark_id}.ru.md"))
        .exists());
    assert_eq!(sandbox_entries(&root), 0);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn autonomy_status_prints_level_and_blockers_and_blocks_level_three() {
    let root = temp_crate("phase46r-autonomy");
    let output = run_ok(&root, &["--autonomy-status"]);
    assert!(output.contains("\"current_level\""));
    assert!(output.contains("\"blockers\""));
    assert!(output.contains("replay"));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn benchmark_does_not_auto_promote() {
    let root = temp_crate("phase46r-no-promote");
    seed_graph(&root);
    let before = fs::read_to_string(root.join("src/probe.rs")).expect("read before");
    run_ok(&root, &["--evolution-benchmark", "2"]);
    let after = fs::read_to_string(root.join("src/probe.rs")).expect("read after");
    assert_eq!(before, after);
    let log = fs::read_to_string(root.join("memory/evolution.jsonl")).expect("log");
    assert!(!log.contains("\"status\":\"promoted\""));
    fs::remove_dir_all(root).expect("cleanup");
}

fn run_ok(root: &PathBuf, args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_eva_runtime_with_task_validator"))
        .args(args)
        .current_dir(root)
        .output()
        .expect("run command");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn latest_candidate_run_id(root: &PathBuf) -> String {
    let mut summaries = fs::read_dir(root.join("memory/candidates"))
        .expect("candidate dir")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.to_string_lossy().ends_with(".summary.json"))
        .collect::<Vec<_>>();
    summaries.sort();
    summaries
        .last()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .map(|name| name.trim_end_matches(".summary.json").to_string())
        .expect("latest candidate run id")
}

fn report_count(root: &PathBuf) -> usize {
    fs::read_dir(root.join("memory/reports"))
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| entry.path().to_string_lossy().ends_with(".ru.md"))
                .count()
        })
        .unwrap_or(0)
}

fn sandbox_entries(root: &PathBuf) -> usize {
    fs::read_dir(root.join("sandboxes"))
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| entry.file_name() != ".gitkeep")
                .count()
        })
        .unwrap_or(0)
}

fn temp_crate(name: &str) -> PathBuf {
    let root = temp_dir(name);
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::create_dir_all(root.join("memory")).expect("create memory");
    fs::create_dir_all(root.join("sandboxes")).expect("create sandboxes");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"phase46r_temp\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write cargo");
    fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("write main");
    fs::write(root.join("src/probe.rs"), "pub fn probe() {}\n").expect("write probe");
    fs::write(root.join("src/runtime_cycle.rs"), "pub fn cycle() {}\n")
        .expect("write runtime cycle");
    root
}

fn seed_graph(root: &PathBuf) {
    fs::write(
        root.join("memory/graph.json"),
        r#"{
  "nodes": [
    {"id":"file:src/probe.rs","kind":"File"},
    {"id":"pattern:function:probe","kind":"Pattern"}
  ],
  "edges": [
    {"from":"pattern:function:probe","to":"file:src/probe.rs","relation":"found_in"}
  ]
}"#,
    )
    .expect("write graph");
}

fn temp_dir(name: &str) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_millis();
    std::env::temp_dir().join(format!("{name}-{}-{millis}", std::process::id()))
}
