use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "evolution_test_support.rs"]
mod evolution_test_support;

use eva_runtime_with_task_validator::contracts::{
    EvolutionLogEntry, EvolutionStatus, MutationContract, MutationKind, MutationObjective,
    MutationPlan, RecombinedHypothesis,
};
use eva_runtime_with_task_validator::{
    compute_quality_for_hypothesis, generate_from_recombined_hypothesis,
    load_recombined_hypotheses, normalized_generated_test_name, print_strategy_portfolio,
    refresh_evolution_policy, refresh_portfolio, refresh_strategy_portfolio, validate_mutation,
    write_report,
};

#[test]
fn portfolio_refresh_rebuilds_from_memory_fixtures() {
    let root = temp_crate("phase52-portfolio-refresh");
    seed_phase52_memory(&root);
    fs::remove_file(root.join("memory/portfolio.json")).expect("remove stale portfolio");

    let portfolio = refresh_portfolio(root.join("memory").to_str().unwrap()).expect("portfolio");
    assert!(portfolio
        .kinds
        .iter()
        .any(|entry| entry.mutation_kind == "addunittest"));
    assert!(portfolio
        .kinds
        .iter()
        .any(|entry| entry.replay_passed_count > 0));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn strategy_portfolio_refresh_works() {
    let root = temp_crate("phase52-strategy-portfolio");
    seed_phase52_memory(&root);

    let portfolio =
        refresh_strategy_portfolio(root.join("memory").to_str().unwrap()).expect("strategy");
    assert!(portfolio
        .strategies
        .iter()
        .any(|entry| entry.strategy == "MetricsReporting"));

    let summary = print_strategy_portfolio(root.join("memory").to_str().unwrap()).expect("print");
    assert!(summary.contains("MetricsReporting"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn evolution_policy_selects_underused_safe_strategy() {
    let root = temp_crate("phase52-policy");
    seed_phase52_memory(&root);

    let policy = refresh_evolution_policy(
        root.to_str().unwrap(),
        root.join("memory").to_str().unwrap(),
        None,
    )
    .expect("policy");
    assert_ne!(policy.selected_strategy, "TestExpansion");
    assert!(policy.allowed_mutation_kinds.iter().all(|kind| {
        !matches!(
            kind.as_str(),
            "appendcomment" | "deletecode" | "rewritefunction" | "freediff" | "dependencyadd"
        )
    }));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn recombination_respects_selected_strategy() {
    let root = temp_crate("phase52-recombination-policy");
    seed_phase52_memory(&root);
    fs::write(
        root.join("memory/evolution_policy.json"),
        r#"{
  "selected_strategy":"MetricsReporting",
  "policy_reason_ru":"seeded policy",
  "allowed_mutation_kinds":["addmetricupdate","addlearningsummaryfield"],
  "preferred_targets":["src/evolution/metrics.rs"],
  "risk_limit":0.25
}"#,
    )
    .expect("policy");

    let hypotheses =
        load_recombined_hypotheses(root.join("memory").to_str().unwrap()).expect("hypotheses");
    assert!(!hypotheses.is_empty());
    assert!(hypotheses.iter().all(|hypothesis| {
        matches!(
            hypothesis.suggested_mutation_kind.as_str(),
            "addmetricupdate" | "addlearningsummaryfield"
        )
    }));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn add_replay_assertion_generates_validator_safe_mutation() {
    let hypothesis = base_hypothesis("addreplayassertion", "tests/evolution_generated_tests.rs");
    let mutation = generate_from_recombined_hypothesis(&hypothesis).expect("mutation");
    validate_mutation(&mutation).expect("valid");
    assert_eq!(mutation.kind, MutationKind::AddReplayAssertion);
}

#[test]
fn add_metric_update_generates_validator_safe_mutation() {
    let hypothesis = base_hypothesis("addmetricupdate", "src/evolution/metrics.rs");
    let mutation = generate_from_recombined_hypothesis(&hypothesis).expect("mutation");
    validate_mutation(&mutation).expect("valid");
    assert_eq!(mutation.kind, MutationKind::AddMetricUpdate);
}

#[test]
fn quality_metrics_reward_novel_useful_mutation() {
    let root = temp_crate("phase52-quality-novel");
    seed_phase52_memory(&root);

    let novel = compute_quality_for_hypothesis(
        root.join("memory").to_str().unwrap(),
        "addreplayassertion",
        "tests/evolution_generated_tests.rs",
        "ReplaySafety",
        &["pattern:new".to_string()],
        &["runtime_to_tests_redirect:src/runtime_cycle.rs".to_string()],
    )
    .expect("novel");
    let saturated = compute_quality_for_hypothesis(
        root.join("memory").to_str().unwrap(),
        "addunittest",
        "tests/evolution_generated_tests.rs",
        "TestExpansion",
        &["success_kind:addunittest:12".to_string()],
        &[],
    )
    .expect("saturated");

    assert!(novel.novelty_score > saturated.novelty_score);
    assert!(novel.quality_score > saturated.quality_score);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn duplicate_payload_lowers_quality_score() {
    let root = temp_crate("phase52-quality-duplicate");
    seed_phase52_memory(&root);

    let clean = compute_quality_for_hypothesis(
        root.join("memory").to_str().unwrap(),
        "addmetricupdate",
        "src/evolution/metrics.rs",
        "MetricsReporting",
        &["pattern:metrics".to_string()],
        &[],
    )
    .expect("clean");
    let duplicate = compute_quality_for_hypothesis(
        root.join("memory").to_str().unwrap(),
        "addmetricupdate",
        "src/evolution/metrics.rs",
        "MetricsReporting",
        &["success_kind:addmetricupdate:3".to_string()],
        &[],
    )
    .expect("duplicate");

    assert!(clean.duplicate_suppression_score > duplicate.duplicate_suppression_score);
    assert!(clean.quality_score > duplicate.quality_score);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn regression_avoidance_raises_quality_score() {
    let root = temp_crate("phase52-quality-regression");
    seed_phase52_memory(&root);

    let redirected = compute_quality_for_hypothesis(
        root.join("memory").to_str().unwrap(),
        "addreplayassertion",
        "tests/evolution_generated_tests.rs",
        "RegressionAvoidance",
        &["pattern:runtime".to_string()],
        &["runtime_to_tests_redirect:src/runtime_cycle.rs".to_string()],
    )
    .expect("redirected");
    let plain = compute_quality_for_hypothesis(
        root.join("memory").to_str().unwrap(),
        "addmetricupdate",
        "src/evolution/metrics.rs",
        "MetricsReporting",
        &["pattern:metrics".to_string()],
        &[],
    )
    .expect("plain");

    assert!(redirected.regression_avoidance_score > plain.regression_avoidance_score);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn russian_report_includes_strategy_and_quality_sections() {
    let root = temp_crate("phase52-report");
    seed_phase52_memory(&root);
    let memory_root = root.join("memory");
    let entry = report_entry();
    let mutation = report_mutation();

    write_report(memory_root.to_str().unwrap(), &entry, &mutation).expect("report");
    let markdown =
        fs::read_to_string(memory_root.join("reports").join("run-report.ru.md")).expect("markdown");
    assert!(markdown.contains("Selected strategy: ReplaySafety"));
    assert!(markdown.contains("Quality score: 1.20"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn review_candidate_prints_quality_fields() {
    let root = temp_crate("phase52-review");
    seed_phase52_memory(&root);

    let output = run_ok(&root, &["--review-candidate", "run-review"]);
    assert!(output.contains("\"selected_strategy\""));
    assert!(output.contains("\"quality_score\""));
    assert!(output.contains("\"novelty_score\""));
    assert!(output.contains("\"useful_delta_score\""));
    assert!(output.contains("\"regression_avoidance_score\""));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn generated_test_names_are_short_valid_and_deterministic_in_phase52() {
    let plan = MutationPlan {
        id: "recombined:src/evolution/metrics.rs:addreplayassertion:tests/evolution_generated_tests.rs"
            .to_string(),
        objective: MutationObjective::ImproveReplayability,
        target_file: "tests/evolution_generated_tests.rs".to_string(),
        mutation_kind: MutationKind::AddReplayAssertion,
        reason: "phase52 test".to_string(),
        expected_gain: 0.5,
        estimated_risk: 0.1,
        evidence_weight: 0.2,
        graph_evidence: vec!["pattern:function:metrics_probe".to_string()],
    };
    let left = normalized_generated_test_name(&plan, "replay");
    let right = normalized_generated_test_name(&plan, "replay");
    assert_eq!(left, right);
    assert!(left.len() <= 80);
    assert!(is_valid_rust_identifier(&left));
}

fn is_valid_rust_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn base_hypothesis(kind: &str, target: &str) -> RecombinedHypothesis {
    RecombinedHypothesis {
        hypothesis_id: format!("recombined:test:{kind}"),
        source_patterns: vec!["pattern:seed".to_string()],
        avoided_risks: vec!["runtime_to_tests_redirect:src/runtime_cycle.rs".to_string()],
        target_objective: "ImproveReliability".to_string(),
        suggested_mutation_kind: kind.to_string(),
        suggested_target: target.to_string(),
        reason_ru: "seed".to_string(),
        portfolio_reason_ru: "portfolio".to_string(),
        selected_strategy: "ReplaySafety".to_string(),
        policy_reason_ru: "policy".to_string(),
        expected_gain: 0.4,
        estimated_risk: 0.1,
        confidence: 0.5,
        diversity_bonus: 0.1,
        saturation_penalty: 0.0,
        repeated_target_penalty: 0.0,
        final_recombination_score: 0.9,
        strategy_bonus: 0.1,
        strategy_saturation_penalty: 0.0,
        quality_bonus: 0.1,
        novelty_score: 0.3,
        useful_delta_score: 0.2,
        duplicate_suppression_score: 0.2,
        regression_avoidance_score: 0.2,
        coverage_proxy_score: 0.2,
        quality_score: 1.1,
        final_strategy_score: 1.2,
    }
}

fn report_entry() -> EvolutionLogEntry {
    EvolutionLogEntry {
        run_id: "run-report".to_string(),
        plan_id: None,
        hypothesis_id: Some("recombined:report".to_string()),
        objective: Some("ImproveReplayability".to_string()),
        graph_evidence: vec!["pattern:seed".to_string()],
        recombined_source_patterns: vec!["pattern:seed".to_string()],
        recombined_avoided_risks: vec!["runtime_to_tests_redirect:src/runtime_cycle.rs".to_string()],
        recombination_reason_ru: Some("reason".to_string()),
        portfolio_reason_ru: Some("portfolio".to_string()),
        selected_strategy: Some("ReplaySafety".to_string()),
        policy_reason_ru: Some("policy".to_string()),
        mutation_class: "useful".to_string(),
        hygiene_warning_ru: None,
        diversity_bonus: 0.1,
        saturation_penalty: 0.0,
        repeated_target_penalty: 0.0,
        final_recombination_score: 0.9,
        strategy_bonus: 0.1,
        strategy_saturation_penalty: 0.0,
        quality_bonus: 0.3,
        novelty_score: 0.3,
        useful_delta_score: 0.2,
        duplicate_suppression_score: 0.2,
        regression_avoidance_score: 0.3,
        coverage_proxy_score: 0.2,
        quality_score: 1.2,
        final_strategy_score: 1.3,
        mutation_id: "mutation:run-report".to_string(),
        mutation_digest: "digest-report".to_string(),
        status: EvolutionStatus::Candidate,
        target_file: "tests/evolution_generated_tests.rs".to_string(),
        mutation_kind: "addreplayassertion".to_string(),
        risk: 0.1,
        score: 8.4,
        useful_change: true,
        non_candidate_reason: None,
        duplicate_rejected: false,
        regression_penalty: 0.0,
        success_bonus: 0.0,
        cargo_check_ok: true,
        cargo_test_ok: true,
        cargo_run_ok: true,
        retained_in_core: false,
        sandbox_destroyed: true,
        stdout_digest: String::new(),
        stderr_digest: String::new(),
        stderr_tail: String::new(),
        timestamp_unix: 7,
    }
}

fn report_mutation() -> MutationContract {
    MutationContract {
        id: "mutation:run-report".to_string(),
        kind: MutationKind::AddReplayAssertion,
        target_file: "tests/evolution_generated_tests.rs".to_string(),
        search: None,
        replace: None,
        append: Some("#[test]\nfn report_seed() { assert!(true); }\n".to_string()),
        reason: "report".to_string(),
        expected_gain: 0.4,
        risk: 0.1,
    }
}

fn seed_phase52_memory(root: &PathBuf) {
    fs::create_dir_all(root.join("memory/patterns")).expect("patterns");
    fs::create_dir_all(root.join("memory/candidates")).expect("candidates");
    fs::create_dir_all(root.join("memory/replays")).expect("replays");
    fs::create_dir_all(root.join("memory/reports")).expect("reports");
    fs::write(
        root.join("memory/graph.json"),
        r#"{
  "nodes": [
    {"id":"file:src/runtime_cycle.rs","kind":"File"},
    {"id":"file:src/evolution/metrics.rs","kind":"File"},
    {"id":"file:src/validator_probe.rs","kind":"File"},
    {"id":"pattern:function:runtime_cycle","kind":"Pattern"},
    {"id":"pattern:function:metrics_probe","kind":"Pattern"},
    {"id":"pattern:function:validator_probe","kind":"Pattern"}
  ],
  "edges": [
    {"from":"pattern:function:runtime_cycle","to":"file:src/runtime_cycle.rs","relation":"found_in"},
    {"from":"pattern:function:metrics_probe","to":"file:src/evolution/metrics.rs","relation":"found_in"},
    {"from":"pattern:function:validator_probe","to":"file:src/validator_probe.rs","relation":"found_in"}
  ]
}"#,
    )
    .expect("graph");
    fs::write(
        root.join("memory/success_patterns.json"),
        r#"[
  {
    "pattern_id":"plan:addunittest",
    "target_area":"tests",
    "target_file":"tests/evolution_generated_tests.rs",
    "mutation_kind":"addunittest",
    "success_count":12,
    "average_score":9.8,
    "bonus":1.0,
    "last_run_id":"seed-addunittest"
  },
  {
    "pattern_id":"plan:addmetricupdate",
    "target_area":"metrics",
    "target_file":"src/evolution/metrics.rs",
    "mutation_kind":"addmetricupdate",
    "success_count":3,
    "average_score":8.5,
    "bonus":0.7,
    "last_run_id":"seed-addmetric"
  }
]"#,
    )
    .expect("success");
    fs::write(
        root.join("memory/regressions.json"),
        r#"[
  {
    "pattern_id":"pattern:appendcomment:src/runtime_cycle.rs",
    "target_area":"src",
    "target_file":"src/runtime_cycle.rs",
    "mutation_kind":"appendcomment",
    "failure_status":"non_useful",
    "fail_count":2,
    "penalty":0.8,
    "last_run_id":"seed-regression"
  }
]"#,
    )
    .expect("regressions");
    fs::write(
        root.join("memory/patterns/local_distilled_patterns.json"),
        r#"{
  "generated_at": 1,
  "top_successful_mutation_kinds": [
    {"key":"addunittest","count":12,"average_score":9.8},
    {"key":"addmetricupdate","count":3,"average_score":8.5}
  ],
  "risky_target_files": [
    {"target_file":"src/runtime_cycle.rs","fail_count":2,"penalty":0.8}
  ],
  "preferred_objectives": [
    {"key":"ImproveReliability","count":4,"average_score":8.0},
    {"key":"ImproveGraphMemory","count":5,"average_score":8.4}
  ]
}"#,
    )
    .expect("distilled");
    fs::write(
        root.join("memory/metrics.json"),
        r#"{
  "total_runs": 24,
  "passed_runs": 22,
  "failed_runs": 2,
  "candidate_count": 10,
  "replay_passed": 4,
  "promoted_count": 3,
  "average_score": 8.0,
  "last_run_id": "run-review"
}"#,
    )
    .expect("metrics");
    fs::write(
        root.join("memory/portfolio.json"),
        r#"{
  "kinds": [
    {
      "mutation_kind":"addunittest",
      "seen_count":20,
      "success_count":18,
      "candidate_count":12,
      "replay_passed_count":4,
      "promoted_count":3,
      "average_score":9.0,
      "saturation_score":0.35,
      "last_used_at":20
    },
    {
      "mutation_kind":"addreplayassertion",
      "seen_count":0,
      "success_count":0,
      "candidate_count":0,
      "replay_passed_count":0,
      "promoted_count":0,
      "average_score":0.0,
      "saturation_score":0.0,
      "last_used_at":0
    },
    {
      "mutation_kind":"addmetricupdate",
      "seen_count":1,
      "success_count":1,
      "candidate_count":0,
      "replay_passed_count":0,
      "promoted_count":0,
      "average_score":8.0,
      "saturation_score":0.0,
      "last_used_at":19
    }
  ]
}"#,
    )
    .expect("portfolio");
    fs::write(
        root.join("memory/evolution.jsonl"),
        format!(
            "{}\n{}\n{}\n",
            serde_json::to_string(&EvolutionLogEntry {
                run_id: "run-metrics".to_string(),
                plan_id: None,
                hypothesis_id: Some("hyp:metrics".to_string()),
                objective: Some("ImproveGraphMemory".to_string()),
                graph_evidence: vec!["pattern:function:metrics_probe".to_string()],
                recombined_source_patterns: vec!["pattern:function:metrics_probe".to_string()],
                recombined_avoided_risks: Vec::new(),
                recombination_reason_ru: Some("metrics".to_string()),
                portfolio_reason_ru: Some("portfolio".to_string()),
                selected_strategy: Some("MetricsReporting".to_string()),
                policy_reason_ru: Some("policy".to_string()),
                mutation_class: "useful".to_string(),
                hygiene_warning_ru: None,
                diversity_bonus: 0.1,
                saturation_penalty: 0.0,
                repeated_target_penalty: 0.0,
                final_recombination_score: 0.8,
                strategy_bonus: 0.2,
                strategy_saturation_penalty: 0.0,
                quality_bonus: 0.2,
                novelty_score: 0.3,
                useful_delta_score: 0.28,
                duplicate_suppression_score: 0.2,
                regression_avoidance_score: 0.1,
                coverage_proxy_score: 0.12,
                quality_score: 1.0,
                final_strategy_score: 1.2,
                mutation_id: "mutation:run-metrics".to_string(),
                mutation_digest: "digest-metrics".to_string(),
                status: EvolutionStatus::Candidate,
                target_file: "src/evolution/metrics.rs".to_string(),
                mutation_kind: "addmetricupdate".to_string(),
                risk: 0.1,
                score: 8.6,
                useful_change: true,
                non_candidate_reason: None,
                duplicate_rejected: false,
                regression_penalty: 0.0,
                success_bonus: 0.0,
                cargo_check_ok: true,
                cargo_test_ok: true,
                cargo_run_ok: true,
                retained_in_core: false,
                sandbox_destroyed: true,
                stdout_digest: String::new(),
                stderr_digest: String::new(),
                stderr_tail: String::new(),
                timestamp_unix: 5,
            })
            .expect("log 1"),
            serde_json::to_string(&EvolutionLogEntry {
                run_id: "run-test".to_string(),
                plan_id: None,
                hypothesis_id: Some("hyp:test".to_string()),
                objective: Some("ImproveTests".to_string()),
                graph_evidence: vec!["pattern:function:validator_probe".to_string()],
                recombined_source_patterns: vec!["success_kind:addunittest:12".to_string()],
                recombined_avoided_risks: Vec::new(),
                recombination_reason_ru: Some("tests".to_string()),
                portfolio_reason_ru: Some("portfolio".to_string()),
                selected_strategy: Some("TestExpansion".to_string()),
                policy_reason_ru: Some("policy".to_string()),
                mutation_class: "useful".to_string(),
                hygiene_warning_ru: None,
                diversity_bonus: 0.0,
                saturation_penalty: 0.3,
                repeated_target_penalty: 0.1,
                final_recombination_score: 0.4,
                strategy_bonus: 0.0,
                strategy_saturation_penalty: 0.2,
                quality_bonus: 0.1,
                novelty_score: 0.05,
                useful_delta_score: 0.2,
                duplicate_suppression_score: 0.1,
                regression_avoidance_score: 0.1,
                coverage_proxy_score: 0.25,
                quality_score: 0.7,
                final_strategy_score: 0.3,
                mutation_id: "mutation:run-test".to_string(),
                mutation_digest: "digest-test".to_string(),
                status: EvolutionStatus::Candidate,
                target_file: "tests/evolution_generated_tests.rs".to_string(),
                mutation_kind: "addunittest".to_string(),
                risk: 0.1,
                score: 7.4,
                useful_change: true,
                non_candidate_reason: None,
                duplicate_rejected: false,
                regression_penalty: 0.0,
                success_bonus: 0.0,
                cargo_check_ok: true,
                cargo_test_ok: true,
                cargo_run_ok: true,
                retained_in_core: false,
                sandbox_destroyed: true,
                stdout_digest: String::new(),
                stderr_digest: String::new(),
                stderr_tail: String::new(),
                timestamp_unix: 6,
            })
            .expect("log 2"),
            serde_json::to_string(&EvolutionLogEntry {
                run_id: "run-review".to_string(),
                ..report_entry()
            })
            .expect("log 3"),
        ),
    )
    .expect("logs");
    fs::write(
        root.join("memory/candidates/run-review.summary.json"),
        r#"{
  "run_id":"run-review",
  "mutation_id":"mutation:run-report",
  "mutation_digest":"digest-report",
  "status":"candidate",
  "target_file":"tests/evolution_generated_tests.rs",
  "mutation_kind":"addreplayassertion",
  "risk":0.1,
  "score":8.4,
  "useful_change":true,
  "non_candidate_reason":null,
  "duplicate_rejected":false,
  "regression_penalty":0.0,
  "success_bonus":0.0,
  "cargo_check_ok":true,
  "cargo_test_ok":true,
  "cargo_run_ok":true,
  "stdout_digest":"",
  "stderr_digest":"",
  "stderr_tail":"",
  "timestamp_unix":7
}"#,
    )
    .expect("summary");
    fs::write(
        root.join("memory/candidates/run-review.mutation.json"),
        serde_json::to_string_pretty(&report_mutation()).expect("mutation"),
    )
    .expect("candidate mutation");
    fs::write(
        root.join("memory/replays/run-review.json"),
        r#"{
  "run_id":"run-review",
  "replay_status":"candidate",
  "matches_stored_summary":true,
  "score":8.4,
  "cargo_check_ok":true,
  "cargo_test_ok":true,
  "cargo_run_ok":true,
  "stdout_digest":"",
  "stderr_digest":"",
  "stderr_tail":"",
  "sandbox_destroyed":true,
  "timestamp_unix":8
}"#,
    )
    .expect("replay");
    write_report(
        root.join("memory").to_str().unwrap(),
        &report_entry().with_run_id("run-review"),
        &report_mutation(),
    )
    .expect("seed report");
}

trait WithRunId {
    fn with_run_id(self, run_id: &str) -> Self;
}

impl WithRunId for EvolutionLogEntry {
    fn with_run_id(mut self, run_id: &str) -> Self {
        self.run_id = run_id.to_string();
        self
    }
}

fn temp_crate(name: &str) -> PathBuf {
    let root = evolution_test_support::unique_evolution_root(name);
    fs::create_dir_all(root.join("src/evolution")).expect("src evolution");
    fs::create_dir_all(root.join("memory")).expect("memory");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"phase52_temp\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("cargo");
    fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("main");
    fs::write(root.join("src/runtime_cycle.rs"), "pub fn cycle() {}\n").expect("runtime");
    fs::write(
        root.join("src/evolution/metrics.rs"),
        "pub fn metrics_probe() -> usize { 1 }\n",
    )
    .expect("metrics");
    fs::write(
        root.join("src/validator_probe.rs"),
        "pub fn validator_probe() -> bool { true }\n",
    )
    .expect("validator");
    root
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
