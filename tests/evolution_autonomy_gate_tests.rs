use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use eva_runtime_with_task_validator::autonomy_status;
use eva_runtime_with_task_validator::contracts::{EvolutionLogEntry, EvolutionStatus};

#[test]
fn autonomy_reaches_level_three_with_clean_metrics() {
    let root = temp_root("autonomy-level3");
    seed_clean_level_three_state(&root);

    let status = autonomy_status(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("autonomy status");
    assert_eq!(status.current_level, 3);
    assert!(status.campaign_mode_allowed);
    assert!(!has_cargo_blocker(&status.blockers));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn cosmetic_non_candidate_run_does_not_downgrade_autonomy() {
    let root = temp_root("autonomy-cosmetic");
    seed_clean_level_three_state(&root);
    append_log(
        &root,
        explicit_log(
            "cosmetic-run",
            "appendcomment",
            EvolutionStatus::Passed,
            false,
            false,
            false,
            false,
            Some("cosmetic_mutation"),
            2.0,
            false,
        ),
    );

    let status = autonomy_status(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("autonomy status");
    assert_eq!(status.current_level, 3);
    assert!(status.campaign_mode_allowed);
    assert!(!has_cargo_blocker(&status.blockers));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn duplicate_rejected_before_sandbox_does_not_downgrade_autonomy() {
    let root = temp_root("autonomy-duplicate");
    seed_clean_level_three_state(&root);
    append_log(
        &root,
        explicit_log(
            "duplicate-run",
            "addunittest",
            EvolutionStatus::Failed,
            false,
            false,
            false,
            true,
            Some("duplicate_bad_mutation"),
            0.0,
            false,
        ),
    );

    let status = autonomy_status(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("autonomy status");
    assert_eq!(status.current_level, 3);
    assert!(status.campaign_mode_allowed);
    assert!(!has_cargo_blocker(&status.blockers));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn legacy_missing_cargo_fields_do_not_downgrade_when_metrics_clean() {
    let root = temp_root("autonomy-legacy");
    seed_legacy_level_three_state(&root);

    let status = autonomy_status(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("autonomy status");
    assert_eq!(status.current_level, 3);
    assert!(status.campaign_mode_allowed);
    assert!(!has_cargo_blocker(&status.blockers));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn campaign_mode_allowed_at_level_three() {
    let root = temp_root("autonomy-campaign-mode");
    seed_clean_level_three_state(&root);

    let status = autonomy_status(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("autonomy status");
    assert_eq!(status.current_level, 3);
    assert!(status.campaign_mode_allowed);
    assert_eq!(status.current_safe_autonomy_level, 3);

    fs::remove_dir_all(root).expect("cleanup");
}

fn seed_clean_level_three_state(root: &PathBuf) {
    fs::create_dir_all(root.join("memory")).expect("memory");
    fs::create_dir_all(root.join("sandboxes")).expect("sandboxes");
    let mut lines = Vec::new();
    for index in 0..16 {
        lines.push(
            serde_json::to_string(&explicit_log(
                &format!("run-{index}"),
                "addunittest",
                EvolutionStatus::Candidate,
                true,
                true,
                true,
                false,
                None,
                10.0,
                false,
            ))
            .expect("serialize log"),
        );
    }
    lines.push(
        serde_json::to_string(&explicit_log(
            "run-promoted",
            "addunittest",
            EvolutionStatus::Promoted,
            true,
            true,
            true,
            false,
            None,
            10.0,
            true,
        ))
        .expect("serialize promoted log"),
    );
    fs::write(
        root.join("memory/evolution.jsonl"),
        format!("{}\n", lines.join("\n")),
    )
    .expect("write logs");
    seed_replays(root, 5);
    seed_regressions(root, 3);
}

fn seed_legacy_level_three_state(root: &PathBuf) {
    fs::create_dir_all(root.join("memory")).expect("memory");
    fs::create_dir_all(root.join("sandboxes")).expect("sandboxes");
    let mut lines = Vec::new();
    for index in 0..16 {
        lines.push(legacy_log(
            &format!("legacy-{index}"),
            false,
            true,
            10.0,
            true,
            true,
            true,
        ));
    }
    lines.push(legacy_log(
        "legacy-promoted",
        true,
        true,
        10.0,
        true,
        true,
        true,
    ));
    fs::write(
        root.join("memory/evolution.jsonl"),
        format!("{}\n", lines.join("\n")),
    )
    .expect("write legacy logs");
    seed_replays(root, 5);
    seed_regressions(root, 3);
}

fn seed_replays(root: &PathBuf, count: usize) {
    fs::create_dir_all(root.join("memory/replays")).expect("replays");
    for index in 0..count {
        fs::write(
            root.join("memory/replays")
                .join(format!("replay-{index}.json")),
            r#"{
  "run_id":"replay",
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

fn seed_regressions(root: &PathBuf, count: usize) {
    let entries = (0..count)
        .map(|index| {
            serde_json::json!({
                "pattern_id": format!("pattern-{index}"),
                "target_area": "tests",
                "target_file": format!("tests/generated_{index}.rs"),
                "mutation_kind": "addunittest",
                "failure_status": "failed",
                "fail_count": 1,
                "penalty": 0.5,
                "last_run_id": format!("run-{index}")
            })
        })
        .collect::<Vec<_>>();
    fs::write(
        root.join("memory/regressions.json"),
        serde_json::to_string_pretty(&entries).expect("serialize regressions"),
    )
    .expect("write regressions");
}

fn append_log(root: &PathBuf, entry: EvolutionLogEntry) {
    let path = root.join("memory/evolution.jsonl");
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let mut merged = existing;
    if !merged.is_empty() && !merged.ends_with('\n') {
        merged.push('\n');
    }
    merged.push_str(&serde_json::to_string(&entry).expect("serialize log"));
    merged.push('\n');
    fs::write(path, merged).expect("append log");
}

fn explicit_log(
    run_id: &str,
    mutation_kind: &str,
    status: EvolutionStatus,
    cargo_check_ok: bool,
    cargo_test_ok: bool,
    cargo_run_ok: bool,
    duplicate_rejected: bool,
    non_candidate_reason: Option<&str>,
    score: f32,
    retained_in_core: bool,
) -> EvolutionLogEntry {
    EvolutionLogEntry {
        run_id: run_id.to_string(),
        plan_id: None,
        hypothesis_id: None,
        objective: Some("ImproveTests".to_string()),
        graph_evidence: Vec::new(),
        recombined_source_patterns: Vec::new(),
        recombined_avoided_risks: Vec::new(),
        recombination_reason_ru: None,
        mutation_id: format!("mutation-{run_id}"),
        mutation_digest: format!("digest-{run_id}"),
        status,
        target_file: "tests/evolution_generated_tests.rs".to_string(),
        mutation_kind: mutation_kind.to_string(),
        risk: 0.1,
        score,
        useful_change: mutation_kind != "appendcomment",
        non_candidate_reason: non_candidate_reason.map(str::to_string),
        duplicate_rejected,
        regression_penalty: 0.0,
        success_bonus: 0.0,
        cargo_check_ok,
        cargo_test_ok,
        cargo_run_ok,
        retained_in_core,
        sandbox_destroyed: true,
        stdout_digest: String::new(),
        stderr_digest: String::new(),
        stderr_tail: String::new(),
        timestamp_unix: 1_700_000_000,
    }
}

fn legacy_log(
    mutation_id: &str,
    retained_in_core: bool,
    accepted: bool,
    score: f32,
    check_passed: bool,
    test_passed: bool,
    run_passed: bool,
) -> String {
    serde_json::json!({
        "mutation": {
            "id": mutation_id,
            "kind": "add_unit_test",
            "target_file": "tests/evolution_generated_tests.rs",
            "search": null,
            "replace": null,
            "append": "#[test]\nfn legacy_test() { assert!(true); }",
            "reason": "legacy",
            "expected_gain": 0.5,
            "risk": 0.1
        },
        "score": {
            "accepted": accepted,
            "score": score,
            "useful_change": true,
            "non_candidate_reason": null,
            "check_passed": check_passed,
            "test_passed": test_passed,
            "run_passed": run_passed
        },
        "retained_in_core": retained_in_core
    })
    .to_string()
}

fn has_cargo_blocker(blockers: &[String]) -> bool {
    blockers
        .iter()
        .any(|blocker| blocker.contains("cargo-gates"))
}

fn temp_root(name: &str) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_millis();
    std::env::temp_dir().join(format!("{name}-{}-{millis}", std::process::id()))
}
