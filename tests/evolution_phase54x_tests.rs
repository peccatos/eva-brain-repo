use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use eva_runtime_with_task_validator::{
    default_corpus_contract, ingest_corpus, list_corpora, load_corpus_patterns,
    load_corpus_summary, suggest_strategy_tasks, validate_corpus_contract, CorpusIngestContract,
};

#[cfg(unix)]
use std::os::unix::fs::symlink;

#[test]
fn corpus_contract_rejects_network_url() {
    let contract = CorpusIngestContract {
        root_path: "https://example.com/repo".to_string(),
        ..default_corpus_contract("https://example.com/repo")
    };
    assert!(validate_corpus_contract(&contract).is_err());
}

#[test]
fn corpus_contract_rejects_unsafe_root() {
    let contract = CorpusIngestContract {
        root_path: "/home".to_string(),
        ..default_corpus_contract("/home")
    };
    assert!(validate_corpus_contract(&contract).is_err());
}

#[test]
#[cfg(unix)]
fn corpus_validator_rejects_path_escape() {
    let root = temp_runtime_root("phase54-path-escape");
    let corpus = seed_corpus_repo(&root);
    let outside = root.join("outside.rs");
    fs::write(&outside, "pub fn escaped() {}\n").expect("outside");
    symlink(&outside, corpus.join("src/escape.rs")).expect("symlink");

    let summary = ingest_corpus(
        root.join("memory").to_str().unwrap(),
        &default_corpus_contract(corpus.to_str().unwrap()),
    )
    .expect("ingest");
    assert!(summary.skipped_files > 0);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn corpus_validator_skips_denied_dirs() {
    let root = temp_runtime_root("phase54-denied");
    let corpus = seed_corpus_repo(&root);
    fs::create_dir_all(corpus.join("target")).expect("target");
    fs::write(corpus.join("target/ignored.rs"), "pub fn ignored() {}\n").expect("ignored");

    let summary = ingest_corpus(
        root.join("memory").to_str().unwrap(),
        &default_corpus_contract(corpus.to_str().unwrap()),
    )
    .expect("ingest");
    assert!(summary.scanned_files < 10);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn corpus_ingestion_does_not_mutate_source_files() {
    let root = temp_runtime_root("phase54-readonly");
    let corpus = seed_corpus_repo(&root);
    let file = corpus.join("src/lib.rs");
    let before = fs::read_to_string(&file).expect("before");

    ingest_corpus(
        root.join("memory").to_str().unwrap(),
        &default_corpus_contract(corpus.to_str().unwrap()),
    )
    .expect("ingest");
    let after = fs::read_to_string(&file).expect("after");
    assert_eq!(before, after);

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn corpus_ingestion_writes_summary_pattern_and_report() {
    let root = temp_runtime_root("phase54-artifacts");
    let corpus = seed_corpus_repo(&root);
    let contract = default_corpus_contract(corpus.to_str().unwrap());

    let summary = ingest_corpus(root.join("memory").to_str().unwrap(), &contract).expect("ingest");
    let dir = root.join("memory/corpus");
    assert!(dir
        .join(format!("{}.summary.json", summary.corpus_id))
        .exists());
    assert!(dir
        .join(format!("{}.patterns.json", summary.corpus_id))
        .exists());
    assert!(dir.join(format!("{}.ru.md", summary.corpus_id)).exists());

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn corpus_extractor_detects_test_validation_cli_and_report_patterns() {
    let root = temp_runtime_root("phase54-patterns");
    let corpus = seed_corpus_repo(&root);
    let summary = ingest_corpus(
        root.join("memory").to_str().unwrap(),
        &default_corpus_contract(corpus.to_str().unwrap()),
    )
    .expect("ingest");
    let patterns = load_corpus_patterns(root.join("memory").to_str().unwrap(), &summary.corpus_id)
        .expect("patterns");

    assert!(patterns
        .detected_patterns
        .iter()
        .any(|value| value == "test_assertion_pattern"));
    assert!(patterns
        .detected_patterns
        .iter()
        .any(|value| value == "validation_guard_pattern"));
    assert!(patterns
        .detected_patterns
        .iter()
        .any(|value| value == "cli_command_pattern"));
    assert!(patterns
        .detected_patterns
        .iter()
        .any(|value| value == "report_writer_pattern"));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn corpus_does_not_store_full_source_content() {
    let root = temp_runtime_root("phase54-no-source-copy");
    let corpus = seed_corpus_repo(&root);
    let marker = "VERY_UNIQUE_SOURCE_MARKER_SHOULD_NOT_BE_STORED";
    fs::write(
        corpus.join("src/marker.rs"),
        format!("pub fn marker() -> &'static str {{ \"{marker}\" }}\n"),
    )
    .expect("marker");
    let summary = ingest_corpus(
        root.join("memory").to_str().unwrap(),
        &default_corpus_contract(corpus.to_str().unwrap()),
    )
    .expect("ingest");
    let summary_json = fs::read_to_string(
        root.join("memory/corpus")
            .join(format!("{}.summary.json", summary.corpus_id)),
    )
    .expect("summary json");
    let patterns_json = fs::read_to_string(
        root.join("memory/corpus")
            .join(format!("{}.patterns.json", summary.corpus_id)),
    )
    .expect("patterns json");
    assert!(!summary_json.contains(marker));
    assert!(!patterns_json.contains(marker));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn strategy_task_suggestions_are_generated_from_corpus_patterns() {
    let root = temp_runtime_root("phase54-suggest");
    let corpus = seed_corpus_repo(&root);
    let summary = ingest_corpus(
        root.join("memory").to_str().unwrap(),
        &default_corpus_contract(corpus.to_str().unwrap()),
    )
    .expect("ingest");

    let tasks = suggest_strategy_tasks(root.join("memory").to_str().unwrap(), &summary.corpus_id)
        .expect("tasks");
    assert!(!tasks.is_empty());
    assert!(tasks.iter().all(|task| !task.auto_promote));
    assert!(tasks
        .iter()
        .all(|task| task.forbidden_targets.contains(&"src/main.rs".to_string())));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn list_corpora_and_corpus_summary_work() {
    let root = temp_runtime_root("phase54-list");
    let corpus = seed_corpus_repo(&root);
    let summary = ingest_corpus(
        root.join("memory").to_str().unwrap(),
        &default_corpus_contract(corpus.to_str().unwrap()),
    )
    .expect("ingest");

    let corpora = list_corpora(root.join("memory").to_str().unwrap()).expect("list");
    assert!(corpora.contains(&summary.corpus_id));
    let loaded = load_corpus_summary(root.join("memory").to_str().unwrap(), &summary.corpus_id)
        .expect("summary");
    assert_eq!(loaded.corpus_id, summary.corpus_id);

    let cli_list = run_ok(&root, &["--list-corpora"]);
    assert!(cli_list.contains(&summary.corpus_id));
    let cli_summary = run_ok(&root, &["--corpus-summary", &summary.corpus_id]);
    assert!(cli_summary.contains("\"corpus_id\""));

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn list_suggested_tasks_works() {
    let root = temp_runtime_root("phase54-list-tasks");
    let corpus = seed_corpus_repo(&root);
    let summary = ingest_corpus(
        root.join("memory").to_str().unwrap(),
        &default_corpus_contract(corpus.to_str().unwrap()),
    )
    .expect("ingest");
    let tasks = suggest_strategy_tasks(root.join("memory").to_str().unwrap(), &summary.corpus_id)
        .expect("tasks");
    let output = run_ok(&root, &["--list-suggested-tasks"]);
    assert!(output.contains(&tasks[0].task_id));

    fs::remove_dir_all(root).expect("cleanup");
}

fn temp_runtime_root(name: &str) -> PathBuf {
    let root = temp_dir(name);
    fs::create_dir_all(root.join("src")).expect("src");
    fs::create_dir_all(root.join("memory")).expect("memory");
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"phase54_temp\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("cargo");
    fs::write(root.join("src/main.rs"), "fn main() {}\n").expect("main");
    root
}

fn seed_corpus_repo(root: &PathBuf) -> PathBuf {
    let corpus = root.join("local_corpus");
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

fn run_ok(root: &PathBuf, args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_eva_runtime_with_task_validator"))
        .args(args)
        .current_dir(root)
        .output()
        .expect("run");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn temp_dir(name: &str) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_millis();
    std::env::temp_dir().join(format!("{name}-{}-{millis}", std::process::id()))
}
