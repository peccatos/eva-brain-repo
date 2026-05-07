use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::contracts::ArtifactAuditReport;
use crate::evolution::memory;

const RUNTIME_PATHS: &[&str] = &[
    "memory/releases/",
    "memory/proof/",
    "memory/corpus/",
    "memory/tasks/suggested/",
    "memory/tasks/adjusted/",
    "memory/tasks/feedback/",
    "memory/bounded_runs/",
    "memory/supervised_runs/",
    "memory/governance/",
    "memory/approvals/",
    "memory/release_proposals/",
    "memory/promotion_queue.json",
    "memory/policy_feedback.json",
    "sandboxes/",
    ".eva-evolution-tests/",
    ".eva-runtime-tests/",
];

pub fn build_artifact_audit(project_root: &str) -> Result<ArtifactAuditReport, String> {
    let root = Path::new(project_root);
    let gitignore = read_gitignore(root);
    let tracked = git_tracked_files(root);
    let mut checked_paths = RUNTIME_PATHS
        .iter()
        .map(|path| path.to_string())
        .collect::<Vec<_>>();
    checked_paths.sort();

    let mut tracked_runtime_artifacts = BTreeSet::new();
    let mut untracked_runtime_artifacts = BTreeSet::new();
    let mut ignored_runtime_artifacts = BTreeSet::new();
    let mut sandbox_leaks = BTreeSet::new();
    let mut unsafe_auto_promote = false;

    for path in &checked_paths {
        let clean = path.trim_end_matches('/');
        let abs = root.join(clean);
        if is_ignored(path, &gitignore) {
            ignored_runtime_artifacts.insert(path.clone());
        }
        for tracked_file in &tracked {
            if tracked_file.ends_with("/.gitkeep") || tracked_file == ".gitkeep" {
                continue;
            }
            if tracked_file == clean || tracked_file.starts_with(&format!("{clean}/")) {
                tracked_runtime_artifacts.insert(tracked_file.clone());
            }
        }
        if abs.exists()
            && !tracked
                .iter()
                .any(|file| file == clean || file.starts_with(clean))
        {
            untracked_runtime_artifacts.insert(path.clone());
        }
        if clean == "sandboxes" && abs.exists() {
            for leak in fs::read_dir(&abs)
                .map_err(|error| format!("failed to read sandboxes: {error}"))?
                .filter_map(Result::ok)
            {
                if leak.file_name() == ".gitkeep" {
                    continue;
                }
                sandbox_leaks.insert(path_to_string(leak.path()));
            }
        }
        if abs.exists() {
            unsafe_auto_promote |= contains_auto_promote_true(&abs);
        }
    }

    let mut recommendations_ru = Vec::new();
    if !tracked_runtime_artifacts.is_empty() {
        recommendations_ru.push(
            "Удалить runtime-артефакты из git index и оставить их под .gitignore.".to_string(),
        );
    }
    if !sandbox_leaks.is_empty() {
        recommendations_ru.push("Очистить sandboxes перед release gate.".to_string());
    }
    if unsafe_auto_promote {
        recommendations_ru
            .push("Проверить runtime-память: найден маркер auto_promote=true.".to_string());
    }
    if recommendations_ru.is_empty() {
        recommendations_ru
            .push("Runtime-артефакты выглядят изолированными и metadata-only.".to_string());
    }

    let should_fail_release =
        unsafe_auto_promote || !tracked_runtime_artifacts.is_empty() || !sandbox_leaks.is_empty();

    Ok(ArtifactAuditReport {
        generated_at: memory::now_unix(),
        checked_paths,
        tracked_runtime_artifacts: tracked_runtime_artifacts.into_iter().collect(),
        untracked_runtime_artifacts: untracked_runtime_artifacts.into_iter().collect(),
        ignored_runtime_artifacts: ignored_runtime_artifacts.into_iter().collect(),
        sandbox_leaks: sandbox_leaks.into_iter().collect(),
        should_fail_release,
        recommendations_ru,
    })
}

pub fn print_artifact_audit(project_root: &str) -> Result<String, String> {
    let report = build_artifact_audit(project_root)?;
    Ok(format!(
        "artifact_audit: checked={} tracked={} untracked={} ignored={} sandbox_leaks={} should_fail_release={}\nrecommendations={}",
        report.checked_paths.len(),
        report.tracked_runtime_artifacts.len(),
        report.untracked_runtime_artifacts.len(),
        report.ignored_runtime_artifacts.len(),
        report.sandbox_leaks.len(),
        report.should_fail_release,
        report.recommendations_ru.join("; ")
    ))
}

pub fn print_artifact_audit_json(project_root: &str) -> Result<String, String> {
    serde_json::to_string_pretty(&build_artifact_audit(project_root)?)
        .map_err(|error| format!("failed to serialize artifact audit: {error}"))
}

fn read_gitignore(root: &Path) -> Vec<String> {
    fs::read_to_string(root.join(".gitignore"))
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.trim_start_matches('/').to_string())
        .collect()
}

fn git_tracked_files(root: &Path) -> BTreeSet<String> {
    let output = Command::new("git")
        .arg("ls-files")
        .arg("-z")
        .current_dir(root)
        .output();
    let Ok(output) = output else {
        return BTreeSet::new();
    };
    if !output.status.success() {
        return BTreeSet::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .split('\0')
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn is_ignored(path: &str, gitignore: &[String]) -> bool {
    let clean = path.trim_start_matches('/').trim_end_matches('/');
    gitignore.iter().any(|pattern| {
        let pattern = pattern.trim_end_matches('/');
        clean == pattern || clean.starts_with(&format!("{pattern}/"))
    })
}

fn contains_auto_promote_true(path: &Path) -> bool {
    if path.is_file() {
        return fs::read_to_string(path)
            .map(|contents| {
                contents.contains("\"auto_promote\": true")
                    || contents.contains("\"auto_promote\":true")
            })
            .unwrap_or(false);
    }
    if !path.is_dir() {
        return false;
    }
    fs::read_dir(path)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .any(|entry| contains_auto_promote_true(&entry.path()))
}

fn path_to_string(path: PathBuf) -> String {
    path.display().to_string()
}
