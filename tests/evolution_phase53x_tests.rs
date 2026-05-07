use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "evolution_test_support.rs"]
mod evolution_test_support;

use eva_runtime_with_task_validator::{
    classify_mutation_kind, classify_mutation_kind_label, compute_quality_for_hypothesis,
    mutation_class_label, refresh_evolution_policy, refresh_portfolio, run_evolution_hygiene,
    MutationKind,
};

#[test]
fn appendcomment_is_classified_as_cosmetic() {
    assert_eq!(
        mutation_class_label(classify_mutation_kind(MutationKind::AppendComment, false)),
        "cosmetic"
    );
}

#[test]
fn useful_mutation_kinds_are_classified_as_useful() {
    assert_eq!(
        mutation_class_label(classify_mutation_kind(MutationKind::AddUnitTest, true)),
        "useful"
    );
    assert_eq!(
        mutation_class_label(classify_mutation_kind(
            MutationKind::AddReplayAssertion,
            true
        )),
        "useful"
    );
    assert_eq!(
        mutation_class_label(classify_mutation_kind(MutationKind::AddMetricUpdate, true)),
        "useful"
    );
}

#[test]
fn unsafe_kinds_are_classified_as_unsafe() {
    assert_eq!(
        mutation_class_label(classify_mutation_kind_label("deletecode", false)),
        "unsafe"
    );
    assert_eq!(
        mutation_class_label(classify_mutation_kind_label("freediff", false)),
        "unsafe"
    );
}

#[test]
fn portfolio_refresh_does_not_count_appendcomment_as_useful_success() {
    let root = temp_hygiene_crate("phase53-portfolio");
    seed_hygiene_memory(&root);

    let portfolio = refresh_portfolio(root.join("memory").to_str().unwrap()).expect("portfolio");
    let append = portfolio
        .kinds
        .iter()
        .find(|entry| entry.mutation_kind == "appendcomment")
        .expect("appendcomment entry");
    assert_eq!(append.useful_success_count, 0);
    assert!(append.cosmetic_count > 0);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn evolution_policy_ignores_cosmetic_success() {
    let root = temp_hygiene_crate("phase53-policy");
    seed_hygiene_memory(&root);

    let policy = refresh_evolution_policy(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        None,
    )
    .expect("policy");
    assert!(!policy
        .allowed_mutation_kinds
        .iter()
        .any(|kind| kind == "appendcomment"));
    assert!(policy.policy_reason_ru.contains("cosmetic_legacy_ignored"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn quality_metrics_penalize_cosmetic_and_legacy_class() {
    let root = temp_hygiene_crate("phase53-quality");
    seed_hygiene_memory(&root);

    let useful = compute_quality_for_hypothesis(
        root.join("memory").to_str().unwrap(),
        "addreplayassertion",
        "tests/evolution_generated_tests.rs",
        "ReplaySafety",
        &[],
        &[],
    )
    .expect("useful");
    let cosmetic = compute_quality_for_hypothesis(
        root.join("memory").to_str().unwrap(),
        "appendcomment",
        "tests/evolution_generated_tests.rs",
        "TestExpansion",
        &[],
        &[],
    )
    .expect("cosmetic");
    let legacy = compute_quality_for_hypothesis(
        root.join("memory").to_str().unwrap(),
        "legacykind",
        "tests/evolution_generated_tests.rs",
        "TestExpansion",
        &[],
        &[],
    )
    .expect("legacy");

    assert!(cosmetic.quality_score < useful.quality_score);
    assert!(legacy.quality_score < useful.quality_score);
    assert!(cosmetic.cosmetic_penalty > 0.0);
    assert!(legacy.legacy_penalty > 0.0);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn hygiene_report_detects_long_generated_test_names() {
    let root = temp_hygiene_crate("phase53-hygiene");
    seed_hygiene_memory(&root);
    write_generated_tests(&root, true, false);

    let report = run_evolution_hygiene(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
    )
    .expect("hygiene");
    assert!(!report.long_generated_test_names.is_empty());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn hygiene_plan_reports_safe_cleanup_action() {
    let root = temp_hygiene_crate("phase53-plan");
    seed_hygiene_memory(&root);
    write_generated_tests(&root, true, false);

    let output = run_ok(&root, &["--hygiene-plan"]);
    assert!(output.contains("--hygiene-fix-generated-tests"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn hygiene_fix_generated_tests_renames_only_long_generated_names() {
    let root = temp_hygiene_crate("phase53-fix");
    seed_hygiene_memory(&root);
    let body = write_generated_tests(&root, true, true);
    let before =
        fs::read_to_string(root.join("tests/evolution_generated_tests.rs")).expect("before");

    let output = run_ok(&root, &["--hygiene-fix-generated-tests"]);
    assert!(output.contains("normalized"));
    let after = fs::read_to_string(root.join("tests/evolution_generated_tests.rs")).expect("after");

    assert_ne!(before, after);
    assert!(after.contains(&body));
    assert!(after.contains("fn eva_generated_short_ok"));
    assert!(!after.contains(&long_generated_name()));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn hygiene_fix_generated_tests_preserves_test_body() {
    let root = temp_hygiene_crate("phase53-body");
    seed_hygiene_memory(&root);
    let body = write_generated_tests(&root, true, false);

    run_ok(&root, &["--hygiene-fix-generated-tests"]);
    let after = fs::read_to_string(root.join("tests/evolution_generated_tests.rs")).expect("after");
    assert!(after.contains(&body));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn hygiene_fix_generated_tests_rollback_works_on_validation_failure() {
    let root = temp_hygiene_crate("phase53-rollback");
    seed_hygiene_memory(&root);
    write_generated_tests(&root, true, false);
    fs::write(root.join("src/lib.rs"), "pub fn broken( { }\n").expect("break lib");
    let before =
        fs::read_to_string(root.join("tests/evolution_generated_tests.rs")).expect("before");

    let output = evolution_test_support::eva_command(&root)
        .args(["--hygiene-fix-generated-tests"])
        .output()
        .expect("run");
    assert!(!output.status.success());
    let after = fs::read_to_string(root.join("tests/evolution_generated_tests.rs")).expect("after");
    assert_eq!(before, after);

    fs::remove_dir_all(root).expect("cleanup");
}

fn seed_hygiene_memory(root: &PathBuf) {
    fs::create_dir_all(root.join("memory")).expect("memory");
    fs::write(
        root.join("memory/evolution.jsonl"),
        concat!(
            "{\"run_id\":\"useful-1\",\"plan_id\":null,\"hypothesis_id\":null,\"objective\":\"ImproveTests\",\"graph_evidence\":[],\"recombined_source_patterns\":[],\"recombined_avoided_risks\":[],\"recombination_reason_ru\":null,\"portfolio_reason_ru\":null,\"selected_strategy\":null,\"policy_reason_ru\":null,\"mutation_class\":\"useful\",\"hygiene_warning_ru\":null,\"diversity_bonus\":0.0,\"saturation_penalty\":0.0,\"repeated_target_penalty\":0.0,\"final_recombination_score\":0.0,\"strategy_bonus\":0.0,\"strategy_saturation_penalty\":0.0,\"quality_bonus\":0.0,\"novelty_score\":0.0,\"useful_delta_score\":0.0,\"duplicate_suppression_score\":0.0,\"regression_avoidance_score\":0.0,\"coverage_proxy_score\":0.0,\"quality_score\":0.0,\"final_strategy_score\":0.0,\"mutation_id\":\"m1\",\"mutation_digest\":\"d1\",\"status\":\"candidate\",\"target_file\":\"tests/evolution_generated_tests.rs\",\"mutation_kind\":\"addunittest\",\"risk\":0.1,\"score\":8.5,\"useful_change\":true,\"non_candidate_reason\":null,\"duplicate_rejected\":false,\"regression_penalty\":0.0,\"success_bonus\":0.0,\"cargo_check_ok\":true,\"cargo_test_ok\":true,\"cargo_run_ok\":true,\"retained_in_core\":false,\"sandbox_destroyed\":true,\"stdout_digest\":\"\",\"stderr_digest\":\"\",\"stderr_tail\":\"\",\"timestamp_unix\":1}\n",
            "{\"run_id\":\"cosmetic-1\",\"plan_id\":null,\"hypothesis_id\":null,\"objective\":\"ImproveReliability\",\"graph_evidence\":[],\"recombined_source_patterns\":[],\"recombined_avoided_risks\":[],\"recombination_reason_ru\":null,\"portfolio_reason_ru\":null,\"selected_strategy\":null,\"policy_reason_ru\":null,\"mutation_class\":\"cosmetic\",\"hygiene_warning_ru\":null,\"diversity_bonus\":0.0,\"saturation_penalty\":0.0,\"repeated_target_penalty\":0.0,\"final_recombination_score\":0.0,\"strategy_bonus\":0.0,\"strategy_saturation_penalty\":0.0,\"quality_bonus\":0.0,\"novelty_score\":0.0,\"useful_delta_score\":0.0,\"duplicate_suppression_score\":0.0,\"regression_avoidance_score\":0.0,\"coverage_proxy_score\":0.0,\"quality_score\":0.0,\"final_strategy_score\":0.0,\"mutation_id\":\"m2\",\"mutation_digest\":\"d2\",\"status\":\"passed\",\"target_file\":\"tests/evolution_generated_tests.rs\",\"mutation_kind\":\"appendcomment\",\"risk\":0.1,\"score\":7.5,\"useful_change\":false,\"non_candidate_reason\":\"cosmetic\",\"duplicate_rejected\":false,\"regression_penalty\":0.0,\"success_bonus\":0.0,\"cargo_check_ok\":true,\"cargo_test_ok\":true,\"cargo_run_ok\":true,\"retained_in_core\":false,\"sandbox_destroyed\":true,\"stdout_digest\":\"\",\"stderr_digest\":\"\",\"stderr_tail\":\"\",\"timestamp_unix\":2}\n"
        ),
    )
    .expect("logs");
    fs::write(
        root.join("memory/metrics.json"),
        r#"{"total_runs":2,"passed_runs":2,"failed_runs":0,"candidate_count":1,"replay_passed":0,"promoted_count":5,"average_score":8.0,"last_run_id":"cosmetic-1"}"#,
    )
    .expect("metrics");
    fs::write(root.join("memory/regressions.json"), "[]").expect("regressions");
    fs::write(root.join("memory/success_patterns.json"), "[]").expect("success");
}

fn temp_hygiene_crate(name: &str) -> PathBuf {
    let root = temp_dir(name);
    fs::create_dir_all(root.join("src")).expect("src");
    fs::create_dir_all(root.join("tests")).expect("tests");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"phase53_temp\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\ndoctest = false\n",
    )
    .expect("cargo");
    fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("main");
    fs::write(root.join("src/lib.rs"), "pub fn probe() -> bool { true }\n").expect("lib");
    root
}

fn write_generated_tests(root: &PathBuf, include_long: bool, include_short: bool) -> String {
    let body = "assert_eq!(2 + 2, 4);";
    let mut file = String::new();
    if include_long {
        file.push_str(&format!(
            "#[test]\nfn {}() {{\n    {}\n}}\n\n",
            long_generated_name(),
            body
        ));
    }
    if include_short {
        file.push_str("#[test]\nfn eva_generated_short_ok() {\n    assert!(true);\n}\n");
    }
    fs::write(root.join("tests/evolution_generated_tests.rs"), file).expect("tests file");
    body.to_string()
}

fn long_generated_name() -> String {
    "eva_generated_this_name_is_far_too_long_for_sane_hygiene_cleanup_and_should_be_shortened_deterministically".to_string()
}

fn run_ok(root: &PathBuf, args: &[&str]) -> String {
    let output = evolution_test_support::eva_command(root)
        .args(args)
        .output()
        .expect("run command");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn temp_dir(name: &str) -> PathBuf {
    evolution_test_support::unique_evolution_root(name)
}
