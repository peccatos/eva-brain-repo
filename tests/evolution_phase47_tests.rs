use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use eva_runtime_with_task_validator::review_candidate;

#[test]
fn review_candidate_prints_useful_candidate_info() {
    let root = temp_crate("phase47-review");
    seed_graph(&root);
    run_ok(&root, &["--evolve-planned"]);
    let run_id = latest_candidate_run_id(&root);

    let output = run_ok(&root, &["--review-candidate", &run_id]);
    assert!(output.contains("\"run_id\""));
    assert!(output.contains("\"mutation_kind\""));
    assert!(output.contains("\"target_file\""));
    assert!(output.contains("\"russian_summary\""));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn candidate_diff_prints_generated_mutation_content() {
    let root = temp_crate("phase47-diff");
    seed_graph(&root);
    run_ok(&root, &["--evolve-planned"]);
    let run_id = latest_candidate_run_id(&root);

    let output = run_ok(&root, &["--candidate-diff", &run_id]);
    assert!(output.contains("tests/evolution_generated_tests.rs"));
    assert!(output.contains("#[test]"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn promotion_readiness_requires_replay_ok() {
    let root = temp_crate("phase47-replay-gate");
    seed_graph(&root);
    run_ok(&root, &["--evolve-planned"]);
    let run_id = latest_candidate_run_id(&root);
    seed_autonomy_ready_state(&root);

    let before = review_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        &run_id,
    )
    .expect("review before replay");
    assert!(!before.promotion_allowed);

    run_ok(&root, &["--replay", &run_id]);
    let after = review_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        &run_id,
    )
    .expect("review after replay");
    assert!(after.promotion_allowed);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn promotion_readiness_rejects_appendcomment_legacy_candidate() {
    let root = temp_crate("phase47-legacy-append");
    seed_legacy_append_candidate(&root);
    seed_autonomy_ready_state(&root);

    let review = review_candidate(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        "legacy-append",
    )
    .expect("review legacy append");
    assert_eq!(review.replay_status, "ok");
    assert!(!review.promotion_allowed);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn review_report_is_written_in_russian() {
    let root = temp_crate("phase47-review-report");
    seed_graph(&root);
    run_ok(&root, &["--evolve-planned"]);
    let run_id = latest_candidate_run_id(&root);
    run_ok(&root, &["--review-candidate", &run_id]);

    let markdown = fs::read_to_string(root.join("memory/reviews").join(format!("{run_id}.ru.md")))
        .expect("read review markdown");
    assert!(markdown.contains("Кандидат"));
    assert!(markdown.contains("Готовность к promotion"));
    let review_json = fs::read_to_string(
        root.join("memory/reviews")
            .join(format!("{run_id}.review.json")),
    )
    .expect("read review json");
    assert!(review_json.contains("\"promotion_allowed\""));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn list_candidates_includes_promotion_readiness() {
    let root = temp_crate("phase47-list");
    seed_graph(&root);
    run_ok(&root, &["--evolve-planned"]);
    let run_id = latest_candidate_run_id(&root);
    let output_before = run_ok(&root, &["--list-candidates"]);
    assert!(output_before.contains("promotion_ready=false"));
    run_ok(&root, &["--replay", &run_id]);
    seed_autonomy_ready_state(&root);
    let output_after = run_ok(&root, &["--list-candidates"]);
    assert!(output_after.contains("replay_status=ok"));
    assert!(output_after.contains("promotion_ready=true"));

    fs::remove_dir_all(root).expect("cleanup");
}

fn seed_legacy_append_candidate(root: &PathBuf) {
    let memory = root.join("memory");
    fs::create_dir_all(memory.join("candidates")).expect("candidates");
    fs::create_dir_all(memory.join("reports")).expect("reports");
    fs::create_dir_all(memory.join("replays")).expect("replays");
    fs::write(
        memory.join("candidates/legacy-append.mutation.json"),
        r#"{
  "id":"legacy-append",
  "kind":"append_comment",
  "target_file":"src/runtime_cycle.rs",
  "search":null,
  "replace":null,
  "append":"// legacy",
  "reason":"legacy append",
  "expected_gain":0.1,
  "risk":0.1
}"#,
    )
    .expect("write mutation");
    fs::write(
        memory.join("candidates/legacy-append.summary.json"),
        r#"{
  "run_id":"legacy-append",
  "mutation_id":"legacy-append",
  "status":"candidate",
  "target_file":"src/runtime_cycle.rs",
  "mutation_kind":"appendcomment",
  "risk":0.1,
  "score":10.0,
  "useful_change":false,
  "cargo_check_ok":true,
  "cargo_test_ok":true,
  "cargo_run_ok":true,
  "stdout_digest":"",
  "stderr_digest":"",
  "stderr_tail":"",
  "timestamp_unix":1
}"#,
    )
    .expect("write summary");
    fs::write(
        memory.join("reports/legacy-append.report.json"),
        r#"{
  "run_id":"legacy-append",
  "status":"candidate",
  "goal_ru":"legacy",
  "selected_plan_ru":"legacy",
  "mutation_ru":"legacy",
  "target_file":"src/runtime_cycle.rs",
  "mutation_kind":"appendcomment",
  "sandbox_ru":"legacy",
  "checks_ru":"legacy",
  "score_ru":"legacy",
  "candidate_ru":"legacy",
  "replay_ru":"Replay пройден и синхронизирован с сохранённым summary.",
  "replay_status":"ok",
  "replay_checked_at":1,
  "risk_ru":"legacy",
  "next_step_ru":"legacy"
}"#,
    )
    .expect("write report json");
    fs::write(memory.join("reports/legacy-append.ru.md"), "# legacy\n").expect("write report md");
    fs::write(
        memory.join("replays/legacy-append.json"),
        r#"{
  "run_id":"legacy-append",
  "replay_status":"candidate",
  "score":10.0,
  "matches_stored_summary":true,
  "cargo_check_ok":true,
  "cargo_test_ok":true,
  "cargo_run_ok":true,
  "stdout_digest":"",
  "stderr_digest":"",
  "stderr_tail":"",
  "sandbox_destroyed":true,
  "timestamp_unix":1
}"#,
    )
    .expect("write replay");
}

fn seed_autonomy_ready_state(root: &PathBuf) {
    let memory = root.join("memory");
    let mut lines = Vec::new();
    for index in 0..10 {
        lines.push(format!(
            "{{\"run_id\":\"seed-{index}\",\"mutation_id\":\"seed-{index}\",\"status\":\"candidate\",\"target_file\":\"tests/evolution_generated_tests.rs\",\"mutation_kind\":\"addunittest\",\"risk\":0.1,\"score\":10.0,\"useful_change\":true,\"duplicate_rejected\":false,\"regression_penalty\":0.0,\"success_bonus\":0.0,\"cargo_check_ok\":true,\"cargo_test_ok\":true,\"cargo_run_ok\":true,\"retained_in_core\":false,\"sandbox_destroyed\":true,\"stdout_digest\":\"\",\"stderr_digest\":\"\",\"stderr_tail\":\"\",\"timestamp_unix\":1}}"
        ));
    }
    let existing = fs::read_to_string(memory.join("evolution.jsonl")).unwrap_or_default();
    let mut merged = existing;
    if !merged.is_empty() && !merged.ends_with('\n') {
        merged.push('\n');
    }
    merged.push_str(&lines.join("\n"));
    merged.push('\n');
    fs::write(memory.join("evolution.jsonl"), merged).expect("write evolution log");
    fs::create_dir_all(memory.join("replays")).expect("replays dir");
    for index in 0..3 {
        fs::write(
            memory
                .join("replays")
                .join(format!("seed-replay-{index}.json")),
            r#"{
  "run_id":"seed",
  "replay_status":"candidate",
  "score":10.0,
  "matches_stored_summary":true,
  "cargo_check_ok":true,
  "cargo_test_ok":true,
  "cargo_run_ok":true,
  "stdout_digest":"",
  "stderr_digest":"",
  "stderr_tail":"",
  "sandbox_destroyed":true,
  "timestamp_unix":1
}"#,
        )
        .expect("write replay");
    }
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

fn temp_crate(name: &str) -> PathBuf {
    let root = temp_dir(name);
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::create_dir_all(root.join("memory")).expect("create memory");
    fs::create_dir_all(root.join("sandboxes")).expect("create sandboxes");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"phase47_temp\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
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
