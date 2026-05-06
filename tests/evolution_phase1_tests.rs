use std::fs;

use eva_runtime_with_task_validator::{
    apply_mutation, score_cycle, validate_mutation, CommandResult, MutationContract, MutationKind,
};

fn base_mutation(target_file: &str) -> MutationContract {
    MutationContract {
        id: "test-mutation".to_string(),
        kind: MutationKind::AppendComment,
        target_file: target_file.to_string(),
        search: None,
        replace: None,
        append: Some("// safe comment".to_string()),
        reason: "test bounded mutation".to_string(),
        expected_gain: 0.1,
        risk: 0.1,
    }
}

#[test]
fn validator_rejects_core_and_path_escape() {
    assert!(validate_mutation(&base_mutation("src/main.rs")).is_err());
    assert!(validate_mutation(&base_mutation("../src/runtime_cycle.rs")).is_err());
    assert!(validate_mutation(&base_mutation("README.md")).is_err());
}

#[test]
fn mutator_applies_only_to_sandbox_file() {
    let root = std::env::temp_dir().join(format!("eva-mutator-test-{}", std::process::id()));
    let source_dir = root.join("src");
    fs::create_dir_all(&source_dir).expect("create temp src");
    fs::write(source_dir.join("probe.rs"), "pub fn probe() {}\n").expect("write probe");

    let mutation = base_mutation("src/probe.rs");
    apply_mutation(root.to_str().expect("utf8 temp path"), &mutation).expect("apply mutation");

    let changed = fs::read_to_string(source_dir.join("probe.rs")).expect("read probe");
    assert!(changed.contains("// safe comment"));

    fs::remove_dir_all(root).expect("cleanup temp dir");
}

#[test]
fn scorer_requires_check_and_test_for_acceptance() {
    let pass = command(true, 10);
    let fail = command(false, 5);

    let accepted = score_cycle(MutationKind::AppendComment, &pass, &pass, Some(&pass));
    assert_eq!(accepted.accepted, true);
    assert_eq!(accepted.score, 2.0);
    assert_eq!(accepted.useful_change, false);
    assert_eq!(
        accepted.non_candidate_reason.as_deref(),
        Some("cosmetic_mutation")
    );

    let rejected = score_cycle(MutationKind::ReplaceText, &pass, &fail, None);
    assert_eq!(rejected.accepted, false);
    assert_eq!(rejected.run_passed, false);
    assert_eq!(rejected.score, 3.0);
    assert_eq!(rejected.useful_change, false);
}

fn command(success: bool, duration_ms: u128) -> CommandResult {
    CommandResult {
        success,
        stdout: String::new(),
        stderr: String::new(),
        duration_ms,
    }
}
