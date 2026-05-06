use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use eva_runtime_with_task_validator::{
    BenchmarkCaseManifest, BenchmarkFailureType, BenchmarkSourceType, RustBugfixCase,
};

const DEFAULT_INPUT_PATH: &str = "benchmarks/rust_cases.json";
const DEFAULT_PREPARED_OUTPUT_PATH: &str = "benchmarks/rust_cases_prepared.json";
const DEFAULT_READY_OUTPUT_PATH: &str = "benchmarks/rust_cases_ready.json";
const COMMAND_OUTPUT_LIMIT: usize = 24_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct DiscoveryManifest {
    cases: Vec<DiscoveryCase>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct DiscoveryCase {
    case_id: String,
    repo_full_name: String,
    repo_url: String,
    license: String,
    default_branch: String,
    source_type: String,
    source_reference: String,
    goal: String,
    local_repo_path: String,
    failure_type: String,
    initial_failure_observed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reproduction_notes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    repo_size_kb: Option<u64>,
    has_tests_or_ci: bool,
    search_score: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct CommandRunRecord {
    command: String,
    status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    exit_code: Option<i32>,
    duration_ms: u64,
    stdout: String,
    stderr: String,
    output_bytes: u64,
    truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct PreparedCase {
    #[serde(flatten)]
    discovery: DiscoveryCase,
    workspace_root: String,
    workspace_prepared: bool,
    cargo_toml_present: bool,
    cargo_metadata: CommandRunRecord,
    cargo_check: CommandRunRecord,
    cargo_test: CommandRunRecord,
    reproducible: bool,
    unreproducible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    observed_failure_type: Option<BenchmarkFailureType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    preparation_error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reproduction_notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct PreparedAggregate {
    total_cases: u64,
    reproducible_cases: u64,
    unreproducible_cases: u64,
    preparation_errors: u64,
    cargo_metadata_failures: u64,
    cargo_check_failures: u64,
    cargo_test_failures: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct PreparedManifest {
    generated_at: String,
    source_manifest: String,
    cases: Vec<PreparedCase>,
    aggregate: PreparedAggregate,
}

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let input_path = args
        .get(0)
        .cloned()
        .unwrap_or_else(|| DEFAULT_INPUT_PATH.to_string());
    let prepared_output_path = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| DEFAULT_PREPARED_OUTPUT_PATH.to_string());
    let ready_output_path = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| DEFAULT_READY_OUTPUT_PATH.to_string());

    let result = run(
        Path::new(&input_path),
        Path::new(&prepared_output_path),
        Path::new(&ready_output_path),
    );

    match result {
        Ok(summary) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&summary)
                    .expect("prepare summary must serialize deterministically")
            );
        }
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

fn run(
    input_path: &Path,
    prepared_output_path: &Path,
    ready_output_path: &Path,
) -> Result<PreparedAggregate, String> {
    let manifest = load_manifest(input_path)?;
    let prepared_cases = manifest.cases.iter().map(prepare_case).collect::<Vec<_>>();
    let prepared_manifest = PreparedManifest {
        generated_at: unix_timestamp_string(),
        source_manifest: input_path.display().to_string(),
        aggregate: aggregate_from_cases(&prepared_cases),
        cases: prepared_cases.clone(),
    };
    write_json(prepared_output_path, &prepared_manifest)?;

    let ready_manifest = build_ready_manifest(&prepared_cases, input_path);
    write_json(ready_output_path, &ready_manifest)?;

    Ok(prepared_manifest.aggregate)
}

fn unix_timestamp_string() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("unix:{seconds}")
}

fn load_manifest(path: &Path) -> Result<DiscoveryManifest, String> {
    let content = fs::read_to_string(path).map_err(|error| {
        format!(
            "failed to read discovery manifest {}: {}",
            path.display(),
            error
        )
    })?;
    serde_json::from_str(&content).map_err(|error| {
        format!(
            "failed to parse discovery manifest {}: {}",
            path.display(),
            error
        )
    })
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create parent dir {}: {}",
                parent.display(),
                error
            )
        })?;
    }
    let content = serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to serialize {}: {}", path.display(), error))?;
    fs::write(path, content)
        .map_err(|error| format!("failed to write {}: {}", path.display(), error))
}

fn prepare_case(discovery: &DiscoveryCase) -> PreparedCase {
    let workspace_root = PathBuf::from(&discovery.local_repo_path);
    let mut preparation_error = None;
    let mut workspace_prepared = false;

    if let Err(error) = ensure_workspace(
        &workspace_root,
        &discovery.repo_url,
        &discovery.default_branch,
    ) {
        preparation_error = Some(error);
    } else {
        workspace_prepared = true;
    }

    let cargo_toml_present = workspace_root.join("Cargo.toml").exists();
    if !cargo_toml_present && preparation_error.is_none() {
        preparation_error = Some("Cargo.toml missing".to_string());
    }
    let cargo_metadata = if workspace_prepared && cargo_toml_present {
        run_cargo_command(
            &workspace_root,
            "metadata",
            &[
                "metadata".to_string(),
                "--no-deps".to_string(),
                "--format-version".to_string(),
                "1".to_string(),
            ],
        )
    } else {
        skipped_command(
            "cargo metadata --no-deps --format-version 1",
            preparation_error
                .clone()
                .unwrap_or_else(|| "Cargo.toml missing".to_string()),
        )
    };
    let cargo_check = if workspace_prepared && cargo_toml_present {
        run_cargo_command(
            &workspace_root,
            "check",
            &locked_cargo_args(&workspace_root, &["check"]),
        )
    } else {
        skipped_command(
            "cargo check",
            preparation_error
                .clone()
                .unwrap_or_else(|| "Cargo.toml missing".to_string()),
        )
    };
    let cargo_test = if workspace_prepared && cargo_toml_present {
        run_cargo_command(
            &workspace_root,
            "test",
            &locked_cargo_args(&workspace_root, &["test"]),
        )
    } else {
        skipped_command(
            "cargo test",
            preparation_error
                .clone()
                .unwrap_or_else(|| "Cargo.toml missing".to_string()),
        )
    };

    let metadata_failed = !record_passed(&cargo_metadata);
    let check_failed = !record_passed(&cargo_check);
    let test_failed = !record_passed(&cargo_test);
    let reproducible = preparation_error.is_none()
        && cargo_toml_present
        && (metadata_failed || check_failed || test_failed);
    let unreproducible = !reproducible;
    let observed_failure_type = if reproducible && (metadata_failed || check_failed) {
        Some(BenchmarkFailureType::CargoCheck)
    } else if reproducible && test_failed {
        Some(BenchmarkFailureType::CargoTest)
    } else {
        None
    };
    let reproduction_notes = build_reproduction_notes(
        workspace_prepared,
        cargo_toml_present,
        &cargo_metadata,
        &cargo_check,
        &cargo_test,
        preparation_error.as_deref(),
        observed_failure_type,
    );

    PreparedCase {
        discovery: discovery.clone(),
        workspace_root: workspace_root.display().to_string(),
        workspace_prepared,
        cargo_toml_present,
        cargo_metadata,
        cargo_check,
        cargo_test,
        reproducible,
        unreproducible,
        observed_failure_type,
        preparation_error,
        reproduction_notes,
    }
}

fn ensure_workspace(
    workspace_root: &Path,
    repo_url: &str,
    default_branch: &str,
) -> Result<(), String> {
    if workspace_root.exists() {
        if workspace_root.join(".git").exists() || workspace_root.join("Cargo.toml").exists() {
            return Ok(());
        }
        if workspace_root
            .read_dir()
            .map_err(|error| format!("failed to inspect {}: {}", workspace_root.display(), error))?
            .next()
            .is_some()
        {
            return Err(format!(
                "local_repo_path exists but is not a repo: {}",
                workspace_root.display()
            ));
        }
    }

    if let Some(parent) = workspace_root.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create parent dir {}: {}",
                parent.display(),
                error
            )
        })?;
    }

    let status = Command::new("git")
        .args([
            "clone",
            "--branch",
            default_branch,
            "--depth",
            "1",
            repo_url,
            workspace_root.to_string_lossy().as_ref(),
        ])
        .status()
        .map_err(|error| format!("failed to spawn git clone: {}", error))?;

    if !status.success() {
        return Err(format!("git clone failed for {}", repo_url));
    }

    Ok(())
}

fn locked_cargo_args(workspace_root: &Path, base: &[&str]) -> Vec<String> {
    let mut args = base
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    if workspace_root.join("Cargo.lock").exists() {
        args.insert(1, "--locked".to_string());
    }
    args
}

fn run_cargo_command(
    workspace_root: &Path,
    command_name: &str,
    args: &[String],
) -> CommandRunRecord {
    let started = Instant::now();
    let mut command = Command::new("cargo");
    command
        .args(args)
        .current_dir(workspace_root)
        .env("CARGO_INCREMENTAL", "0")
        .env("CARGO_BUILD_JOBS", "1")
        .env("CARGO_PROFILE_DEV_DEBUG", "0")
        .env("CARGO_PROFILE_TEST_DEBUG", "0")
        .env("CARGO_TERM_COLOR", "never");

    match command.output() {
        Ok(output) => {
            let stdout = truncate_utf8(&output.stdout, COMMAND_OUTPUT_LIMIT);
            let stderr = truncate_utf8(&output.stderr, COMMAND_OUTPUT_LIMIT);
            let status = if output.status.success() {
                "ok"
            } else {
                "error"
            }
            .to_string();
            CommandRunRecord {
                command: format!("cargo {}", command_name),
                status,
                exit_code: output.status.code(),
                duration_ms: started.elapsed().as_millis().max(1) as u64,
                output_bytes: (output.stdout.len() + output.stderr.len()) as u64,
                truncated: output.stdout.len() > COMMAND_OUTPUT_LIMIT
                    || output.stderr.len() > COMMAND_OUTPUT_LIMIT,
                stdout,
                stderr,
            }
        }
        Err(error) => CommandRunRecord {
            command: format!("cargo {}", command_name),
            status: "error".to_string(),
            exit_code: None,
            duration_ms: started.elapsed().as_millis().max(1) as u64,
            output_bytes: error.to_string().len() as u64,
            truncated: false,
            stdout: String::new(),
            stderr: error.to_string(),
        },
    }
}

fn skipped_command(command: &str, reason: String) -> CommandRunRecord {
    CommandRunRecord {
        command: command.to_string(),
        status: "skipped".to_string(),
        exit_code: None,
        duration_ms: 0,
        output_bytes: reason.len() as u64,
        truncated: false,
        stdout: String::new(),
        stderr: reason,
    }
}

fn truncate_utf8(bytes: &[u8], limit: usize) -> String {
    let take = bytes.len().min(limit);
    String::from_utf8_lossy(&bytes[..take]).to_string()
}

fn record_passed(record: &CommandRunRecord) -> bool {
    record.status == "ok" && record.exit_code == Some(0)
}

fn record_failed(record: &CommandRunRecord) -> bool {
    record.status == "error"
}

fn build_reproduction_notes(
    workspace_prepared: bool,
    cargo_toml_present: bool,
    cargo_metadata: &CommandRunRecord,
    cargo_check: &CommandRunRecord,
    cargo_test: &CommandRunRecord,
    preparation_error: Option<&str>,
    observed_failure_type: Option<BenchmarkFailureType>,
) -> Option<String> {
    let mut notes = Vec::new();
    notes.push(format!("workspace_prepared={workspace_prepared}"));
    notes.push(format!("cargo_toml_present={cargo_toml_present}"));
    notes.push(format!(
        "cargo_metadata={}:{}",
        cargo_metadata.status,
        cargo_metadata.exit_code.unwrap_or(-1)
    ));
    notes.push(format!(
        "cargo_check={}:{}",
        cargo_check.status,
        cargo_check.exit_code.unwrap_or(-1)
    ));
    notes.push(format!(
        "cargo_test={}:{}",
        cargo_test.status,
        cargo_test.exit_code.unwrap_or(-1)
    ));
    if let Some(error) = preparation_error {
        notes.push(format!("preparation_error={error}"));
    }
    if let Some(failure_type) = observed_failure_type {
        notes.push(format!("observed_failure_type={failure_type:?}"));
    }
    if notes.is_empty() {
        None
    } else {
        Some(notes.join("; "))
    }
}

fn aggregate_from_cases(cases: &[PreparedCase]) -> PreparedAggregate {
    PreparedAggregate {
        total_cases: cases.len() as u64,
        reproducible_cases: cases.iter().filter(|case| case.reproducible).count() as u64,
        unreproducible_cases: cases.iter().filter(|case| case.unreproducible).count() as u64,
        preparation_errors: cases
            .iter()
            .filter(|case| case.preparation_error.is_some())
            .count() as u64,
        cargo_metadata_failures: cases
            .iter()
            .filter(|case| record_failed(&case.cargo_metadata))
            .count() as u64,
        cargo_check_failures: cases
            .iter()
            .filter(|case| record_failed(&case.cargo_check))
            .count() as u64,
        cargo_test_failures: cases
            .iter()
            .filter(|case| record_failed(&case.cargo_test))
            .count() as u64,
    }
}

fn build_ready_manifest(cases: &[PreparedCase], source_manifest: &Path) -> BenchmarkCaseManifest {
    let ready_cases = cases
        .iter()
        .filter(|case| case.reproducible && case.preparation_error.is_none())
        .filter_map(|case| {
            let failure_type = case.observed_failure_type?;
            Some(RustBugfixCase {
                case_id: case.discovery.case_id.clone(),
                repo_full_name: case.discovery.repo_full_name.clone(),
                repo_url: case.discovery.repo_url.clone(),
                license: case.discovery.license.clone(),
                default_branch: case.discovery.default_branch.clone(),
                source_type: match failure_type {
                    BenchmarkFailureType::CargoCheck => BenchmarkSourceType::CompileFailure,
                    BenchmarkFailureType::CargoTest => BenchmarkSourceType::FailingTest,
                    BenchmarkFailureType::RuntimeFailure => BenchmarkSourceType::CiFailure,
                    BenchmarkFailureType::AssertionFailure => BenchmarkSourceType::FailingTest,
                    BenchmarkFailureType::Unknown => BenchmarkSourceType::LocalFixture,
                },
                source_reference: "local_reproduction".to_string(),
                goal: case.discovery.goal.clone(),
                local_repo_path: case.discovery.local_repo_path.clone(),
                failure_type,
                initial_failure_observed: true,
                reproduction_notes: case
                    .reproduction_notes
                    .clone()
                    .or_else(|| case.discovery.reproduction_notes.clone()),
            })
        })
        .collect::<Vec<_>>();

    BenchmarkCaseManifest {
        version: Some(source_manifest.display().to_string()),
        benchmark_mode: None,
        cases: ready_cases,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn discovery_case() -> DiscoveryCase {
        DiscoveryCase {
            case_id: "case_001_sample_repo".to_string(),
            repo_full_name: "sample/repo".to_string(),
            repo_url: "https://github.com/sample/repo".to_string(),
            license: "MIT".to_string(),
            default_branch: "main".to_string(),
            source_type: "github_search".to_string(),
            source_reference: "repository_discovery".to_string(),
            goal: "fix regression".to_string(),
            local_repo_path: "C:/benchmarks/sample_repo".to_string(),
            failure_type: "unknown".to_string(),
            initial_failure_observed: false,
            reproduction_notes: Some("sample".to_string()),
            repo_size_kb: Some(1200),
            has_tests_or_ci: true,
            search_score: 2.0,
        }
    }

    fn command_record(status: &str, exit_code: Option<i32>) -> CommandRunRecord {
        CommandRunRecord {
            command: "cargo check".to_string(),
            status: status.to_string(),
            exit_code,
            duration_ms: 1,
            stdout: String::new(),
            stderr: String::new(),
            output_bytes: 0,
            truncated: false,
        }
    }

    #[test]
    fn classify_check_failure_as_compile_failure() {
        let prepared = PreparedCase {
            workspace_root: "C:/benchmarks/sample_repo".to_string(),
            workspace_prepared: true,
            cargo_toml_present: true,
            cargo_metadata: command_record("ok", Some(0)),
            cargo_check: command_record("error", Some(1)),
            cargo_test: command_record("ok", Some(0)),
            reproducible: true,
            unreproducible: false,
            observed_failure_type: Some(BenchmarkFailureType::CargoCheck),
            preparation_error: None,
            reproduction_notes: None,
            discovery: discovery_case(),
        };

        let ready =
            build_ready_manifest(&[prepared], Path::new("benchmarks/rust_cases_ready.json"));
        assert_eq!(ready.cases.len(), 1);
        assert!(matches!(
            ready.cases[0].source_type,
            BenchmarkSourceType::CompileFailure
        ));
    }

    #[test]
    fn unreproducible_cases_are_excluded_from_ready_manifest() {
        let prepared = PreparedCase {
            discovery: discovery_case(),
            workspace_root: "C:/benchmarks/sample_repo".to_string(),
            workspace_prepared: true,
            cargo_toml_present: true,
            cargo_metadata: command_record("ok", Some(0)),
            cargo_check: command_record("ok", Some(0)),
            cargo_test: command_record("ok", Some(0)),
            reproducible: false,
            unreproducible: true,
            observed_failure_type: None,
            preparation_error: None,
            reproduction_notes: None,
        };

        let ready =
            build_ready_manifest(&[prepared], Path::new("benchmarks/rust_cases_ready.json"));
        assert!(ready.cases.is_empty());
    }
}
