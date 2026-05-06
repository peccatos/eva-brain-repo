use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use eva_runtime_with_task_validator::contracts::{EvolutionLogEntry, EvolutionStatus};
use eva_runtime_with_task_validator::evolution::memory::{
    load_candidate_summary, maybe_store_candidate, store_candidate, CandidateSummary,
};
use eva_runtime_with_task_validator::{
    check_promotion_gate, ingest_repo_patterns, promote_candidate, replay_candidate, score_cycle,
    update_graph_for_evolution, CommandResult, MutationContract, MutationKind,
};

fn mutation(target_file: &str, risk: f32) -> MutationContract {
    MutationContract {
        id: "phase25-test-mutation".to_string(),
        kind: MutationKind::AppendComment,
        target_file: target_file.to_string(),
        search: None,
        replace: None,
        append: Some("// phase25 test mutation".to_string()),
        reason: "test phase 2.5 candidate flow".to_string(),
        expected_gain: 0.1,
        risk,
    }
}

fn useful_mutation(target_file: &str, risk: f32) -> MutationContract {
    MutationContract {
        id: "phase25-useful-mutation".to_string(),
        kind: MutationKind::ReplaceText,
        target_file: target_file.to_string(),
        search: Some("pub fn probe() {}".to_string()),
        replace: Some("pub fn probe() {}\n".to_string()),
        append: None,
        reason: "test useful candidate flow".to_string(),
        expected_gain: 0.2,
        risk,
    }
}

fn add_unit_test_mutation(target_file: &str, append: &str, risk: f32) -> MutationContract {
    MutationContract {
        id: "phase25-add-unit-test".to_string(),
        kind: MutationKind::AddUnitTest,
        target_file: target_file.to_string(),
        search: None,
        replace: None,
        append: Some(append.to_string()),
        reason: "test new-file promotion flow".to_string(),
        expected_gain: 0.8,
        risk,
    }
}

fn log_entry(
    run_id: &str,
    score: f32,
    status: EvolutionStatus,
    useful_change: bool,
    mutation_kind: &str,
    non_candidate_reason: Option<&str>,
) -> EvolutionLogEntry {
    EvolutionLogEntry {
        run_id: run_id.to_string(),
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
        mutation_id: "phase25-test-mutation".to_string(),
        mutation_digest: "phase25-digest".to_string(),
        status,
        target_file: "src/probe.rs".to_string(),
        mutation_kind: mutation_kind.to_string(),
        risk: 0.1,
        score,
        useful_change,
        non_candidate_reason: non_candidate_reason.map(str::to_string),
        duplicate_rejected: false,
        regression_penalty: 0.0,
        success_bonus: 0.0,
        cargo_check_ok: score >= 3.0,
        cargo_test_ok: score >= 7.0,
        cargo_run_ok: score >= 10.0,
        retained_in_core: false,
        sandbox_destroyed: true,
        stdout_digest: "stdout".to_string(),
        stderr_digest: "stderr".to_string(),
        stderr_tail: String::new(),
        timestamp_unix: 1,
    }
}

#[test]
fn candidate_stored_only_when_score_at_least_five() {
    let root = temp_dir("candidate-store");
    fs::create_dir_all(&root).expect("create temp memory");

    let accepted = log_entry(
        "accepted",
        5.0,
        EvolutionStatus::Candidate,
        true,
        "replacetext",
        None,
    );
    assert!(maybe_store_candidate(
        root.to_str().unwrap(),
        &accepted,
        &useful_mutation("src/probe.rs", 0.1)
    )
    .expect("store candidate"));
    assert!(root.join("candidates/accepted.mutation.json").exists());
    assert!(root.join("candidates/accepted.summary.json").exists());

    let failed = log_entry(
        "failed",
        4.9,
        EvolutionStatus::Failed,
        true,
        "replacetext",
        None,
    );
    assert!(!maybe_store_candidate(
        root.to_str().unwrap(),
        &failed,
        &useful_mutation("src/probe.rs", 0.1)
    )
    .expect("skip failed candidate"));
    assert!(!root.join("candidates/failed.mutation.json").exists());

    let cosmetic = log_entry(
        "cosmetic",
        5.0,
        EvolutionStatus::Passed,
        false,
        "appendcomment",
        Some("cosmetic_mutation"),
    );
    assert!(!maybe_store_candidate(
        root.to_str().unwrap(),
        &cosmetic,
        &mutation("src/probe.rs", 0.1)
    )
    .expect("skip cosmetic candidate"));
    assert!(!root.join("candidates/cosmetic.mutation.json").exists());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn replay_reruns_candidate_in_fresh_sandbox() {
    let project = temp_crate("replay-project");
    let memory = temp_dir("replay-memory");
    fs::create_dir_all(&memory).expect("create memory");

    let entry = log_entry(
        "replay-run",
        10.0,
        EvolutionStatus::Candidate,
        true,
        "replacetext",
        None,
    );
    store_candidate(
        memory.to_str().unwrap(),
        &entry,
        &useful_mutation("src/probe.rs", 0.1),
    )
    .expect("store candidate");

    replay_candidate(
        project.to_str().unwrap(),
        memory.to_str().unwrap(),
        "replay-run",
    )
    .expect("replay candidate");

    let replay_path = memory.join("replays/replay-run.json");
    assert!(replay_path.exists());
    let replay = fs::read_to_string(replay_path).expect("read replay");
    assert!(replay.contains("\"matches_stored_summary\": true"));
    assert!(!project
        .join("src/probe.rs")
        .read_to_string_lossy()
        .contains("phase25"));

    fs::remove_dir_all(project).expect("cleanup project");
    fs::remove_dir_all(memory).expect("cleanup memory");
}

#[test]
fn promotion_rejects_high_risk_and_core_targets() {
    assert!(!check_promotion_gate(&mutation("src/probe.rs", 0.1), 10.0).allowed);
    assert!(!check_promotion_gate(&mutation("src/probe.rs", 0.26), 10.0).allowed);
    assert!(!check_promotion_gate(&mutation("src/core/belief_state.rs", 0.1), 10.0).allowed);
    assert!(!check_promotion_gate(&mutation("src/main.rs", 0.1), 10.0).allowed);
    assert!(!check_promotion_gate(&mutation("src/lib.rs", 0.1), 10.0).allowed);
}

#[test]
fn graph_updates_after_successful_evolution() {
    let memory = temp_dir("graph-memory");
    let entry = log_entry(
        "graph-run",
        10.0,
        EvolutionStatus::Candidate,
        true,
        "replacetext",
        None,
    );

    update_graph_for_evolution(memory.to_str().unwrap(), &entry).expect("update graph");

    let graph = fs::read_to_string(memory.join("graph.json")).expect("read graph");
    assert!(graph.contains("mutation:phase25-test-mutation"));
    assert!(graph.contains("file:src/probe.rs"));
    assert!(graph.contains("score_band:high"));

    fs::remove_dir_all(memory).expect("cleanup memory");
}

#[test]
fn repo_ingestion_does_not_mutate_source_repo() {
    let project = temp_crate("ingest-project");
    let memory = temp_dir("ingest-memory");
    let before = fs::read_to_string(project.join("src/probe.rs")).expect("read before");

    ingest_repo_patterns(project.to_str().unwrap(), memory.to_str().unwrap()).expect("ingest repo");

    let after = fs::read_to_string(project.join("src/probe.rs")).expect("read after");
    assert_eq!(before, after);
    let graph = fs::read_to_string(memory.join("graph.json")).expect("read graph");
    assert!(graph.contains("pattern:function:probe"));

    fs::remove_dir_all(project).expect("cleanup project");
    fs::remove_dir_all(memory).expect("cleanup memory");
}

#[test]
fn candidate_summary_round_trips() {
    let memory = temp_dir("summary-memory");
    let entry = log_entry(
        "summary-run",
        7.0,
        EvolutionStatus::Candidate,
        true,
        "replacetext",
        None,
    );
    store_candidate(
        memory.to_str().unwrap(),
        &entry,
        &useful_mutation("src/probe.rs", 0.1),
    )
    .expect("store candidate");

    let summary: CandidateSummary =
        load_candidate_summary(memory.to_str().unwrap(), "summary-run").expect("load summary");
    assert_eq!(summary.run_id, "summary-run");
    assert_eq!(summary.score, 7.0);

    fs::remove_dir_all(memory).expect("cleanup memory");
}

#[test]
fn append_comment_score_is_cosmetic_and_non_candidate() {
    let pass = command(true, 10);
    let score = score_cycle(MutationKind::AppendComment, &pass, &pass, Some(&pass));

    assert_eq!(score.score, 2.0);
    assert_eq!(score.useful_change, false);
    assert_eq!(
        score.non_candidate_reason.as_deref(),
        Some("cosmetic_mutation")
    );
}

#[test]
fn useful_replace_text_score_can_be_candidate() {
    let pass = command(true, 10);
    let score = score_cycle(MutationKind::ReplaceText, &pass, &pass, Some(&pass));

    assert_eq!(score.score, 10.0);
    assert_eq!(score.useful_change, true);
    assert_eq!(score.non_candidate_reason, None);
}

#[test]
fn promote_add_unittest_creates_missing_target_file() {
    let project = temp_crate("promote-new-file");
    let memory = temp_dir("promote-new-file-memory");
    fs::create_dir_all(&memory).expect("create memory");

    let run_id = "promote-new-file-run";
    let entry = log_entry(
        run_id,
        10.0,
        EvolutionStatus::Candidate,
        true,
        "addunittest",
        None,
    );
    let mutation = add_unit_test_mutation(
        "tests/evolution_generated_tests.rs",
        "#[test]\nfn generated_phase25_test() {\n    assert_eq!(2 + 2, 4);\n}",
        0.1,
    );
    store_candidate(memory.to_str().unwrap(), &entry, &mutation).expect("store candidate");

    assert!(!project.join("tests/evolution_generated_tests.rs").exists());
    promote_candidate(project.to_str().unwrap(), memory.to_str().unwrap(), run_id)
        .expect("promote new file candidate");
    let created = fs::read_to_string(project.join("tests/evolution_generated_tests.rs"))
        .expect("read created test file");
    assert!(created.contains("generated_phase25_test"));

    fs::remove_dir_all(project).expect("cleanup project");
    fs::remove_dir_all(memory).expect("cleanup memory");
}

#[test]
fn failed_new_file_promotion_removes_created_file() {
    let project = temp_crate("promote-new-file-rollback");
    let memory = temp_dir("promote-new-file-rollback-memory");
    fs::create_dir_all(&memory).expect("create memory");

    let run_id = "promote-new-file-rollback-run";
    let entry = log_entry(
        run_id,
        10.0,
        EvolutionStatus::Candidate,
        true,
        "addunittest",
        None,
    );
    let mutation = add_unit_test_mutation(
        "tests/evolution_generated_tests.rs",
        "#[test]\nfn generated_phase25_broken_test( {\n    assert!(true);\n}",
        0.1,
    );
    store_candidate(memory.to_str().unwrap(), &entry, &mutation).expect("store candidate");

    let error = promote_candidate(project.to_str().unwrap(), memory.to_str().unwrap(), run_id)
        .expect_err("promotion should fail validation");
    assert!(error.contains("cargo fmt failed"));
    assert!(!project.join("tests/evolution_generated_tests.rs").exists());

    fs::remove_dir_all(project).expect("cleanup project");
    fs::remove_dir_all(memory).expect("cleanup memory");
}

#[test]
fn failed_existing_file_promotion_restores_backup() {
    let project = temp_crate("promote-existing-file-rollback");
    let memory = temp_dir("promote-existing-file-rollback-memory");
    fs::create_dir_all(&memory).expect("create memory");
    fs::create_dir_all(project.join("tests")).expect("create tests dir");
    let target = project.join("tests/evolution_generated_tests.rs");
    let original = "#[test]\nfn existing_test() {\n    assert!(true);\n}\n";
    fs::write(&target, original).expect("write existing test");

    let run_id = "promote-existing-file-rollback-run";
    let entry = log_entry(
        run_id,
        10.0,
        EvolutionStatus::Candidate,
        true,
        "addunittest",
        None,
    );
    let mutation = add_unit_test_mutation(
        "tests/evolution_generated_tests.rs",
        "#[test]\nfn generated_phase25_broken_test( {\n    assert!(true);\n}",
        0.1,
    );
    store_candidate(memory.to_str().unwrap(), &entry, &mutation).expect("store candidate");

    let error = promote_candidate(project.to_str().unwrap(), memory.to_str().unwrap(), run_id)
        .expect_err("promotion should fail validation");
    assert!(error.contains("cargo fmt failed"));
    assert_eq!(
        fs::read_to_string(&target).expect("read restored file"),
        original
    );

    fs::remove_dir_all(project).expect("cleanup project");
    fs::remove_dir_all(memory).expect("cleanup memory");
}

#[test]
fn promotion_rejects_forbidden_new_files() {
    let project = temp_crate("promote-forbidden-new-file");
    let memory = temp_dir("promote-forbidden-new-file-memory");
    fs::create_dir_all(&memory).expect("create memory");

    let run_id = "promote-forbidden-new-file-run";
    let entry = log_entry(
        run_id,
        10.0,
        EvolutionStatus::Candidate,
        true,
        "addunittest",
        None,
    );
    let mutation = add_unit_test_mutation(
        "src/main.rs",
        "#[test]\nfn forbidden_generated_test() {\n    assert!(true);\n}",
        0.1,
    );
    store_candidate(memory.to_str().unwrap(), &entry, &mutation).expect("store candidate");

    let error = promote_candidate(project.to_str().unwrap(), memory.to_str().unwrap(), run_id)
        .expect_err("forbidden target should be rejected");
    assert!(error.contains("forbidden"));
    assert!(!project
        .join("src/main.rs")
        .read_to_string_lossy()
        .contains("forbidden_generated_test"));

    fs::remove_dir_all(project).expect("cleanup project");
    fs::remove_dir_all(memory).expect("cleanup memory");
}

fn temp_crate(name: &str) -> PathBuf {
    let root = temp_dir(name);
    fs::create_dir_all(root.join("src")).expect("create crate src");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"eva_phase25_temp\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write cargo toml");
    fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("write main");
    fs::write(root.join("src/probe.rs"), "pub fn probe() {}\n").expect("write probe");
    root
}

fn temp_dir(name: &str) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_millis();
    std::env::temp_dir().join(format!("{name}-{}-{millis}", std::process::id()))
}

trait ReadToStringLossy {
    fn read_to_string_lossy(&self) -> String;
}

impl ReadToStringLossy for Path {
    fn read_to_string_lossy(&self) -> String {
        fs::read_to_string(self).unwrap_or_default()
    }
}

fn command(success: bool, duration_ms: u128) -> CommandResult {
    CommandResult {
        success,
        stdout: String::new(),
        stderr: String::new(),
        duration_ms,
    }
}
