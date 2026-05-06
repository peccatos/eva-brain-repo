#[path = "tool_test_support.rs"]
mod tool_test_support;

use eva_runtime_with_task_validator::{
    BenchmarkCaseManifest, BenchmarkFailureType, BenchmarkRunner, BenchmarkSourceType,
    DiscoveryConfig, GithubToolExecutor, RustBugfixCase,
};
use std::fs;
use std::path::Path;
use tool_test_support::unique_tool_root;

#[test]
fn runner_records_reproducible_case_and_mutation_attempt() {
    let root = unique_tool_root("benchmark_runner_case");
    fs::create_dir_all(root.join("src")).expect("src dir");
    fs::create_dir_all(root.join("tests")).expect("tests dir");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"demo_case\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("cargo toml");
    fs::write(
        root.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }\n",
    )
    .expect("lib");
    fs::write(
        root.join("tests/failing_case.rs"),
        "use demo_case::add;\n#[test]\nfn fails_now() { assert_eq!(add(1, 1), 3); }\n",
    )
    .expect("test");

    let manifest = BenchmarkCaseManifest {
        version: Some("demo".to_string()),
        benchmark_mode: Some("repair_activation".to_string()),
        cases: vec![RustBugfixCase {
            case_id: "case_001_demo".to_string(),
            repo_full_name: "demo/case".to_string(),
            repo_url: "https://github.com/demo/case".to_string(),
            license: "MIT".to_string(),
            default_branch: "main".to_string(),
            source_type: BenchmarkSourceType::FailingTest,
            source_reference: "fixture".to_string(),
            goal: "fix failing test".to_string(),
            local_repo_path: root.display().to_string(),
            failure_type: BenchmarkFailureType::CargoTest,
            initial_failure_observed: true,
            reproduction_notes: None,
        }],
    };

    let report = BenchmarkRunner::default()
        .run_manifest(&manifest, Some(1))
        .expect("run manifest");

    assert_eq!(report.aggregate.total_cases, 1);
    assert_eq!(report.aggregate.reproducible_cases, 1);
    assert!(report.aggregate.mutation_attempt_rate > 0.0);
    assert_eq!(report.cases[0].mutations_attempted, 1);
}

#[test]
fn github_fixture_search_filters_non_permissive_repos() {
    let root = unique_tool_root("github_fixture_search");
    fs::create_dir_all(&root).expect("root dir");
    let fixture_path = root.join("fixture.json");
    fs::write(
        &fixture_path,
        r#"{
  "items": [
    {
      "full_name": "demo/mit-crate",
      "html_url": "https://github.com/demo/mit-crate",
      "description": "Rust crate with tests",
      "stargazers_count": 10,
      "size": 1000,
      "forks_count": 1,
      "open_issues_count": 1,
      "default_branch": "main",
      "archived": false,
      "disabled": false,
      "license": { "spdx_id": "MIT" }
    },
    {
      "full_name": "demo/gpl-crate",
      "html_url": "https://github.com/demo/gpl-crate",
      "description": "Rust crate with tests",
      "stargazers_count": 20,
      "size": 1200,
      "forks_count": 1,
      "open_issues_count": 1,
      "default_branch": "main",
      "archived": false,
      "disabled": false,
      "license": { "spdx_id": "GPL-3.0" }
    }
  ]
}"#,
    )
    .expect("fixture");

    let config = DiscoveryConfig {
        language: "Rust".to_string(),
        query: "rust cargo toml tests".to_string(),
        license_allowlist: vec!["MIT".to_string()],
        exclude_full_names: Vec::new(),
        exclude_names: Vec::new(),
        min_repo_size_kb: None,
        max_repo_size_kb: Some(5000),
        target_repo_size_kb: 3000,
        require_tests_or_ci: true,
        min_stars: 1,
        max_results: 10,
        output_manifest_path: "benchmarks/rust_cases.json".to_string(),
    };

    let results = GithubToolExecutor::new()
        .search_repositories(&config, Some(Path::new(&fixture_path)))
        .expect("fixture search");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].full_name, "demo/mit-crate");
}
