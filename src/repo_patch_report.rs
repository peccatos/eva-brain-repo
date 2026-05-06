use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const DEFAULT_REPORT_PATH: &str = "./eva_output/report.md";
pub const DEFAULT_MACHINE_SUMMARY_PATH: &str = "./eva_output/summary.json";
pub const DEFAULT_MAX_CHANGED_FILES: usize = 10;
pub const MAX_CHANGED_FILES_LIMIT: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoPatchCliConfig {
    pub repo_url: String,
    pub branch: Option<String>,
    pub max_changed_files: usize,
    pub report_path: String,
    pub machine_summary_path: String,
}

impl RepoPatchCliConfig {
    pub fn parse_from_iter<I, S>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut repo_url = None;
        let mut branch = None;
        let mut max_changed_files = DEFAULT_MAX_CHANGED_FILES;
        let mut report_path = DEFAULT_REPORT_PATH.to_string();
        let mut machine_summary_path = DEFAULT_MACHINE_SUMMARY_PATH.to_string();

        let mut args = args.into_iter().map(Into::into).peekable();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--repo" => {
                    let Some(value) = args.next() else {
                        return Err("--repo requires a repository URL".to_string());
                    };
                    repo_url = Some(value);
                }
                "--branch" => {
                    let Some(value) = args.next() else {
                        return Err("--branch requires a branch name".to_string());
                    };
                    branch = normalize_branch(Some(value));
                }
                "--max-changed-files" => {
                    let Some(value) = args.next() else {
                        return Err("--max-changed-files requires an integer value".to_string());
                    };
                    max_changed_files = parse_max_changed_files(&value)?;
                }
                "--report-path" => {
                    let Some(value) = args.next() else {
                        return Err("--report-path requires a file path".to_string());
                    };
                    report_path = value;
                }
                "--machine-summary-path" => {
                    let Some(value) = args.next() else {
                        return Err("--machine-summary-path requires a file path".to_string());
                    };
                    machine_summary_path = value;
                }
                unknown if unknown.starts_with("--repo=") => {
                    repo_url = Some(unknown.trim_start_matches("--repo=").trim().to_string());
                }
                unknown if unknown.starts_with("--branch=") => {
                    branch =
                        normalize_branch(Some(unknown.trim_start_matches("--branch=").to_string()));
                }
                unknown if unknown.starts_with("--max-changed-files=") => {
                    max_changed_files = parse_max_changed_files(
                        unknown.trim_start_matches("--max-changed-files="),
                    )?;
                }
                unknown if unknown.starts_with("--report-path=") => {
                    report_path = unknown.trim_start_matches("--report-path=").to_string();
                }
                unknown if unknown.starts_with("--machine-summary-path=") => {
                    machine_summary_path = unknown
                        .trim_start_matches("--machine-summary-path=")
                        .to_string();
                }
                unknown => {
                    return Err(format!("unsupported repository report argument: {unknown}"));
                }
            }
        }

        let Some(repo_url) = repo_url else {
            return Err("missing required --repo <REPO_URL>".to_string());
        };
        if repo_url.trim().is_empty() {
            return Err("--repo must not be empty".to_string());
        }

        Ok(Self {
            repo_url,
            branch,
            max_changed_files,
            report_path,
            machine_summary_path,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepoPatchStatus {
    Ok,
    Partial,
    Fail,
}

impl RepoPatchStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            RepoPatchStatus::Ok => "ok",
            RepoPatchStatus::Partial => "partial",
            RepoPatchStatus::Fail => "fail",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepoChangeType {
    Create,
    Update,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoChangedFile {
    pub path: String,
    pub language: String,
    pub change_type: RepoChangeType,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoPatchMachineSummary {
    pub repo_url: String,
    pub report_path: String,
    pub status: RepoPatchStatus,
    pub summary: String,
    pub changed_files: Vec<RepoChangedFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoPatchExecution {
    pub repo_url: String,
    pub report_path: String,
    pub machine_summary_path: String,
    pub status: RepoPatchStatus,
    pub summary: String,
    pub changed_files: Vec<RepoChangedFile>,
}

impl RepoPatchExecution {
    pub fn stdout_output(&self) -> String {
        let changed_files = if self.changed_files.is_empty() {
            "(none)".to_string()
        } else {
            self.changed_files
                .iter()
                .map(|file| format!("- {}", file.path))
                .collect::<Vec<_>>()
                .join("\n")
        };

        format!(
            "[repo]\n{}\n\n[report]\n{}\n\n[changed_files]\n{}\n\n[status]\n{}",
            self.repo_url,
            self.report_path,
            changed_files,
            self.status.as_str()
        )
    }
}

#[derive(Debug, Clone)]
struct RepoPatchFileSection {
    path: String,
    language: String,
    change_type: RepoChangeType,
    reason: String,
    final_contents: String,
    report_code: String,
}

#[derive(Debug, Clone)]
struct RepoAnalysis {
    repo_name: String,
    has_root_manifest: bool,
    has_package_manifest: bool,
    has_workspace_manifest: bool,
    has_rust_ci_workflow: bool,
    gitignore_has_target: bool,
    has_rust_tests: bool,
    top_level_items: Vec<String>,
}

impl RepoAnalysis {
    fn describe(&self) -> String {
        let mut parts = Vec::new();
        if self.has_package_manifest {
            parts.push("корневой crate".to_string());
        } else if self.has_workspace_manifest {
            parts.push("workspace".to_string());
        } else if self.has_root_manifest {
            parts.push("Rust-манифест без package-секции".to_string());
        } else {
            parts.push("репозиторий без корневого Cargo.toml".to_string());
        }
        if self.has_rust_ci_workflow {
            parts.push("CI уже есть".to_string());
        } else {
            parts.push("CI отсутствует".to_string());
        }
        if self.has_rust_tests {
            parts.push("тесты уже есть".to_string());
        } else {
            parts.push("тестов в корне нет".to_string());
        }
        if self.gitignore_has_target {
            parts.push("target уже исключён".to_string());
        } else {
            parts.push("target ещё не исключён".to_string());
        }
        parts.join(", ")
    }
}

struct RepoPatchOutcome {
    status: RepoPatchStatus,
    summary: String,
    changed_files: Vec<RepoChangedFile>,
    report_markdown: String,
}

pub fn should_run_repo_patch_mode<I, S>(args: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter().any(|arg| {
        let value = arg.as_ref();
        value == "--repo" || value.starts_with("--repo=")
    })
}

pub fn run_repo_patch_report(config: &RepoPatchCliConfig) -> Result<RepoPatchExecution, String> {
    let report_path = resolve_path(&config.report_path)?;
    let machine_summary_path = resolve_path(&config.machine_summary_path)?;
    let workspace_root = report_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let repo_root = workspace_root
        .join("repos")
        .join(repo_slug(&config.repo_url));

    let outcome = run_repo_patch_pipeline(config, &repo_root);
    write_report(&report_path, &outcome.report_markdown)?;
    let machine_summary = RepoPatchMachineSummary {
        repo_url: config.repo_url.clone(),
        report_path: config.report_path.clone(),
        status: outcome.status,
        summary: outcome.summary.clone(),
        changed_files: outcome.changed_files.clone(),
    };
    write_machine_summary(&machine_summary_path, &machine_summary)?;

    Ok(RepoPatchExecution {
        repo_url: config.repo_url.clone(),
        report_path: config.report_path.clone(),
        machine_summary_path: config.machine_summary_path.clone(),
        status: outcome.status,
        summary: outcome.summary,
        changed_files: outcome.changed_files,
    })
}

fn run_repo_patch_pipeline(config: &RepoPatchCliConfig, repo_root: &Path) -> RepoPatchOutcome {
    let analysis = match clone_and_analyze_repo(config, repo_root) {
        Ok(analysis) => analysis,
        Err(error) => {
            let summary = format!("EVA не смогла подготовить изменения: {error}.");
            return RepoPatchOutcome {
                status: RepoPatchStatus::Fail,
                summary: summary.clone(),
                changed_files: Vec::new(),
                report_markdown: render_report(&config.repo_url, &summary, &[]),
            };
        }
    };

    if !analysis.has_root_manifest {
        let summary = format!(
            "EVA нашла репозиторий {}, но корневой Cargo.toml отсутствует, поэтому безопасный Rust-патч не подготовлен.",
            analysis.repo_name
        );
        return RepoPatchOutcome {
            status: RepoPatchStatus::Fail,
            summary: summary.clone(),
            changed_files: Vec::new(),
            report_markdown: render_report(&config.repo_url, &summary, &[]),
        };
    }

    let plan = build_patch_plan(&analysis, config.max_changed_files);
    if plan.is_empty() {
        let summary = format!(
            "EVA проанализировала репозиторий {}: {}. Безопасного малого патча в текущей политике не нашлось.",
            analysis.repo_name,
            analysis.describe()
        );
        return RepoPatchOutcome {
            status: RepoPatchStatus::Partial,
            summary: summary.clone(),
            changed_files: Vec::new(),
            report_markdown: render_report(&config.repo_url, &summary, &[]),
        };
    }

    let apply_error = apply_patch_plan(repo_root, &plan).err();
    let validation_error = if apply_error.is_none() {
        validate_patch_plan(repo_root, &plan).err()
    } else {
        None
    };
    let status = if apply_error.is_none() && validation_error.is_none() {
        RepoPatchStatus::Ok
    } else {
        RepoPatchStatus::Partial
    };
    let summary = match (&apply_error, &validation_error) {
        (None, None) => format!(
            "EVA проанализировала репозиторий {} и подготовила {} конкретных изменения: {}.",
            analysis.repo_name,
            plan.len(),
            summarize_reasons(&plan)
        ),
        (Some(error), _) => format!(
            "EVA подготовила {} изменения для репозитория {}, но часть записи завершилась с ошибкой: {}.",
            plan.len(),
            analysis.repo_name,
            error
        ),
        (None, Some(error)) => format!(
            "EVA подготовила {} изменения для репозитория {}, но проверка кодовой правки не прошла: {}.",
            plan.len(),
            analysis.repo_name,
            error
        ),
    };
    let changed_files = plan
        .iter()
        .map(|section| RepoChangedFile {
            path: section.path.clone(),
            language: section.language.clone(),
            change_type: section.change_type,
            reason: section.reason.clone(),
        })
        .collect::<Vec<_>>();

    RepoPatchOutcome {
        status,
        summary: summary.clone(),
        changed_files,
        report_markdown: render_report(&config.repo_url, &summary, &plan),
    }
}

fn clone_and_analyze_repo(
    config: &RepoPatchCliConfig,
    repo_root: &Path,
) -> Result<RepoAnalysis, String> {
    prepare_clone_directory(repo_root)?;
    clone_repository(repo_root, &config.repo_url, config.branch.as_deref())?;
    analyze_repo(repo_root)
}

fn prepare_clone_directory(repo_root: &Path) -> Result<(), String> {
    if let Some(parent) = repo_root.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create clone root {}: {}",
                parent.display(),
                error
            )
        })?;
    }
    if repo_root.exists() {
        fs::remove_dir_all(repo_root).map_err(|error| {
            format!(
                "failed to reset clone dir {}: {}",
                repo_root.display(),
                error
            )
        })?;
    }
    Ok(())
}

fn clone_repository(repo_root: &Path, repo_url: &str, branch: Option<&str>) -> Result<(), String> {
    let local_source = Path::new(repo_url);
    if local_source.exists() {
        copy_directory_recursively(local_source, repo_root)?;
        return Ok(());
    }

    let mut command = Command::new("git");
    command.arg("clone").arg("--depth").arg("1");
    if let Some(branch) = branch {
        command.arg("--branch").arg(branch);
    }
    command.arg(repo_url).arg(repo_root);

    let output = command
        .output()
        .map_err(|error| format!("failed to run git clone: {}", error))?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        Err(format!("git clone failed for {}", repo_url))
    } else {
        Err(format!("git clone failed for {}: {}", repo_url, stderr))
    }
}

fn copy_directory_recursively(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination)
        .map_err(|error| format!("failed to create {}: {}", destination.display(), error))?;

    for entry in fs::read_dir(source)
        .map_err(|error| format!("failed to read {}: {}", source.display(), error))?
    {
        let entry =
            entry.map_err(|error| format!("failed to inspect {}: {}", source.display(), error))?;
        let path = entry.path();
        let name = entry.file_name();
        if name == OsStr::new(".git") {
            continue;
        }
        let target = destination.join(&name);
        if path.is_dir() {
            copy_directory_recursively(&path, &target)?;
        } else {
            fs::copy(&path, &target).map_err(|error| {
                format!(
                    "failed to copy {} to {}: {}",
                    path.display(),
                    target.display(),
                    error
                )
            })?;
        }
    }

    Ok(())
}

fn analyze_repo(repo_root: &Path) -> Result<RepoAnalysis, String> {
    let cargo_toml_path = repo_root.join("Cargo.toml");
    let cargo_toml = fs::read_to_string(&cargo_toml_path).ok();
    let has_root_manifest = cargo_toml.is_some();
    let has_package_manifest = cargo_toml
        .as_deref()
        .map(|contents| contents.contains("[package]"))
        .unwrap_or(false);
    let has_workspace_manifest = cargo_toml
        .as_deref()
        .map(|contents| contents.contains("[workspace]"))
        .unwrap_or(false);
    let has_rust_ci_workflow = has_rust_ci_workflow(repo_root)?;
    let gitignore_has_target = gitignore_has_target(repo_root)?;
    let has_rust_tests = has_rust_tests(repo_root)?;
    let top_level_items = fs::read_dir(repo_root)
        .map_err(|error| {
            format!(
                "failed to read repo root {}: {}",
                repo_root.display(),
                error
            )
        })?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect::<Vec<_>>();
    let repo_name = repo_root
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("repository")
        .to_string();

    Ok(RepoAnalysis {
        repo_name,
        has_root_manifest,
        has_package_manifest,
        has_workspace_manifest,
        has_rust_ci_workflow,
        gitignore_has_target,
        has_rust_tests,
        top_level_items,
    })
}

fn has_rust_ci_workflow(repo_root: &Path) -> Result<bool, String> {
    let workflow_root = repo_root.join(".github").join("workflows");
    if !workflow_root.exists() {
        return Ok(false);
    }

    let entries = fs::read_dir(&workflow_root).map_err(|error| {
        format!(
            "failed to read workflow directory {}: {}",
            workflow_root.display(),
            error
        )
    })?;

    for entry in entries.flatten() {
        let path = entry.path();
        let extension = path.extension().and_then(OsStr::to_str).unwrap_or_default();
        if !matches!(extension, "yml" | "yaml") {
            continue;
        }
        let contents = fs::read_to_string(&path)
            .unwrap_or_default()
            .to_ascii_lowercase();
        if contents.contains("cargo check")
            || contents.contains("cargo test")
            || contents.contains("dtolnay/rust-toolchain")
            || contents.contains("actions-rs/toolchain")
        {
            return Ok(true);
        }
    }

    Ok(false)
}

fn gitignore_has_target(repo_root: &Path) -> Result<bool, String> {
    let path = repo_root.join(".gitignore");
    if !path.exists() {
        return Ok(false);
    }
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {}", path.display(), error))?;
    Ok(contents
        .lines()
        .any(|line| matches!(line.trim(), "target" | "target/" | "/target" | "/target/")))
}

fn has_rust_tests(repo_root: &Path) -> Result<bool, String> {
    let tests_root = repo_root.join("tests");
    if !tests_root.exists() {
        return Ok(false);
    }

    let entries = fs::read_dir(&tests_root).map_err(|error| {
        format!(
            "failed to read tests dir {}: {}",
            tests_root.display(),
            error
        )
    })?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .extension()
            .and_then(OsStr::to_str)
            .map(|ext| ext.eq_ignore_ascii_case("rs"))
            .unwrap_or(false)
        {
            return Ok(true);
        }
    }

    Ok(false)
}

fn build_patch_plan(
    analysis: &RepoAnalysis,
    max_changed_files: usize,
) -> Vec<RepoPatchFileSection> {
    let mut plan = Vec::new();

    if !analysis.has_rust_ci_workflow {
        plan.push(RepoPatchFileSection {
            path: ".github/workflows/rust-ci.yml".to_string(),
            language: "yaml".to_string(),
            change_type: RepoChangeType::Create,
            reason: "Добавлен базовый CI для cargo check и cargo test.".to_string(),
            final_contents: rust_ci_workflow(),
            report_code: rust_ci_workflow(),
        });
    }

    if !analysis.gitignore_has_target {
        plan.push(RepoPatchFileSection {
            path: ".gitignore".to_string(),
            language: "gitignore".to_string(),
            change_type: if analysis
                .top_level_items
                .iter()
                .any(|item| item == ".gitignore")
            {
                RepoChangeType::Update
            } else {
                RepoChangeType::Create
            },
            reason: "Исключён каталог target, чтобы build-артефакты не попадали в репозиторий."
                .to_string(),
            final_contents: rust_gitignore_block(),
            report_code: rust_gitignore_block(),
        });
    }

    if analysis.has_package_manifest && !analysis.has_rust_tests {
        plan.push(RepoPatchFileSection {
            path: "tests/eva_smoke.rs".to_string(),
            language: "rust".to_string(),
            change_type: RepoChangeType::Create,
            reason: "Добавлен минимальный smoke test, который не лезет в API проекта и не ломает компиляцию сам по себе.".to_string(),
            final_contents: smoke_test_contents(),
            report_code: smoke_test_contents(),
        });
    }

    plan.truncate(max_changed_files.min(MAX_CHANGED_FILES_LIMIT));
    plan
}

fn apply_patch_plan(repo_root: &Path, plan: &[RepoPatchFileSection]) -> Result<(), String> {
    for section in plan {
        let path = repo_root.join(&section.path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {}", parent.display(), error))?;
        }

        let final_contents = if section.path == ".gitignore" && path.exists() {
            merge_gitignore(&path, &section.final_contents)?
        } else {
            section.final_contents.clone()
        };
        fs::write(&path, final_contents)
            .map_err(|error| format!("failed to write {}: {}", path.display(), error))?;
    }
    Ok(())
}

fn validate_patch_plan(repo_root: &Path, plan: &[RepoPatchFileSection]) -> Result<(), String> {
    if plan
        .iter()
        .any(|section| section.path == "tests/eva_smoke.rs")
    {
        let output = Command::new("cargo")
            .args(["test", "--test", "eva_smoke", "--no-run"])
            .current_dir(repo_root)
            .env("CARGO_TERM_COLOR", "never")
            .output()
            .map_err(|error| format!("failed to run cargo test validation: {}", error))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let details = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                "cargo test --test eva_smoke --no-run failed".to_string()
            };
            return Err(details);
        }
    }

    Ok(())
}

fn merge_gitignore(path: &Path, block: &str) -> Result<String, String> {
    let existing = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {}", path.display(), error))?;
    let mut merged = existing.trim_end().to_string();
    if !merged.is_empty() {
        merged.push_str("\n\n");
    }
    merged.push_str(block);
    merged.push('\n');
    Ok(merged)
}

fn rust_ci_workflow() -> String {
    r#"name: Rust CI

on:
  push:
  pull_request:

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Cargo check
        run: cargo check --all-targets
      - name: Cargo test
        run: cargo test
"#
    .to_string()
}

fn rust_gitignore_block() -> String {
    r#"# EVA: Rust build output
/target/
"#
    .to_string()
}

fn smoke_test_contents() -> String {
    r#"#[test]
fn cargo_manifest_exists() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    assert!(
        manifest.exists(),
        "Cargo.toml must exist at {}",
        manifest.display()
    );
}
"#
    .to_string()
}

fn summarize_reasons(plan: &[RepoPatchFileSection]) -> String {
    plan.iter()
        .map(|section| section.reason.trim_end_matches('.').to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_report(repo_url: &str, summary: &str, sections: &[RepoPatchFileSection]) -> String {
    let changed_files = if sections.is_empty() {
        "- (none)".to_string()
    } else {
        sections
            .iter()
            .map(|section| format!("- {}", section.path))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let file_sections = sections
        .iter()
        .map(|section| {
            format!(
                "## {}\n```{}\n{}```",
                section.path,
                section.language,
                ensure_trailing_newline(&section.report_code)
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    if file_sections.is_empty() {
        format!(
            "# EVA Report\n\n## Repo\n{}\n\n## Summary\n{}\n\n## Changed files\n{}\n",
            repo_url, summary, changed_files
        )
    } else {
        format!(
            "# EVA Report\n\n## Repo\n{}\n\n## Summary\n{}\n\n## Changed files\n{}\n\n{}",
            repo_url, summary, changed_files, file_sections
        )
    }
}

fn ensure_trailing_newline(code: &str) -> String {
    if code.ends_with('\n') {
        code.to_string()
    } else {
        format!("{code}\n")
    }
}

fn write_report(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create report dir {}: {}",
                parent.display(),
                error
            )
        })?;
    }
    fs::write(path, contents)
        .map_err(|error| format!("failed to write report {}: {}", path.display(), error))
}

fn write_machine_summary(path: &Path, summary: &RepoPatchMachineSummary) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create machine summary dir {}: {}",
                parent.display(),
                error
            )
        })?;
    }
    let contents = serde_json::to_string_pretty(summary)
        .map_err(|error| format!("failed to serialize summary {}: {}", path.display(), error))?;
    fs::write(path, contents)
        .map_err(|error| format!("failed to write summary {}: {}", path.display(), error))
}

fn resolve_path(raw: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        Ok(path)
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .map_err(|error| format!("failed to resolve path {}: {}", raw, error))
    }
}

fn normalize_branch(branch: Option<String>) -> Option<String> {
    branch.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("default") {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn parse_max_changed_files(value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| format!("invalid --max-changed-files value: {value}"))?;
    if parsed == 0 || parsed > MAX_CHANGED_FILES_LIMIT {
        return Err(format!(
            "--max-changed-files must be between 1 and {}",
            MAX_CHANGED_FILES_LIMIT
        ));
    }
    Ok(parsed)
}

fn repo_slug(repo_url: &str) -> String {
    let trimmed = repo_url.trim().trim_end_matches('/');
    let without_suffix = trimmed.strip_suffix(".git").unwrap_or(trimmed);
    let source = Path::new(without_suffix)
        .file_name()
        .and_then(OsStr::to_str)
        .filter(|value| !value.is_empty())
        .unwrap_or(without_suffix);

    let mut slug = String::with_capacity(source.len());
    let mut prev_was_sep = false;
    for ch in source.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            prev_was_sep = false;
        } else if !prev_was_sep {
            slug.push('_');
            prev_was_sep = true;
        }
    }
    slug.trim_matches('_').to_string()
}
