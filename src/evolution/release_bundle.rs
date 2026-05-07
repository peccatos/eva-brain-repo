use std::fs;
use std::path::Path;

use crate::contracts::{ReleaseBundle, ReleaseManifest, ReleasePreflightReport, RollbackManifest};
use crate::evolution::changelog::render_release_changelog;
use crate::evolution::{
    memory,
    release_preflight::{
        build_release_preflight, load_release_candidate_context as load_release_context,
        release_id_from_preflight, ReleaseCandidateContext,
    },
};

pub fn build_release_bundle(
    project_root: &str,
    memory_root: &str,
    run_id: &str,
) -> Result<ReleaseBundle, String> {
    let preflight = build_release_preflight(project_root, memory_root, run_id)?;
    if !preflight.allowed {
        return Err(format!(
            "release preflight blocked bundle: {}",
            preflight.blockers.join(", ")
        ));
    }
    let release_id = release_id_from_preflight(&preflight);
    let context = load_release_context(project_root, memory_root, run_id)?;
    let paths = release_paths(memory_root, &release_id);
    let manifest = ReleaseManifest {
        release_id: release_id.clone(),
        source_run_id: run_id.to_string(),
        target_file: preflight.target_file.clone(),
        mutation_kind: preflight.mutation_kind.clone(),
        mutation_class: preflight.mutation_class.clone(),
        replay_status: preflight.replay_status.clone(),
        approved: preflight.approved,
        auto_promote: false,
        source_mutated: false,
        rollback_available: true,
        changelog_available: true,
        created_at: context.generated_at,
    };
    let rollback = RollbackManifest {
        release_id: release_id.clone(),
        source_run_id: run_id.to_string(),
        target_file: preflight.target_file.clone(),
        rollback_type: "metadata_only".to_string(),
        rollback_available: true,
        original_candidate_report_path: context.candidate_report_path.clone(),
        notes: vec![
            "metadata-only release bundle".to_string(),
            "no source mutation performed".to_string(),
            "rollback manifest generated".to_string(),
        ],
        created_at: context.generated_at,
    };
    let bundle = ReleaseBundle {
        release_id: release_id.clone(),
        source_run_id: run_id.to_string(),
        target_file: preflight.target_file.clone(),
        mutation_kind: preflight.mutation_kind.clone(),
        mutation_class: preflight.mutation_class.clone(),
        score: preflight.score,
        risk: preflight.risk,
        replay_status: preflight.replay_status.clone(),
        approval_status: approval_status(&context),
        promotion_queue_state: preflight.promotion_queue_state.clone(),
        candidate_report_path: context.candidate_report_path.clone(),
        preflight_report_path: paths.preflight_report_path.clone(),
        release_manifest_path: paths.manifest_path.clone(),
        rollback_manifest_path: paths.rollback_path.clone(),
        changelog_path: paths.changelog_path.clone(),
        candidate_diff_summary: context
            .candidate_diff_summary
            .clone()
            .unwrap_or_else(|| "(missing diff)".to_string()),
        safety_notes: vec![
            "metadata-only release bundle".to_string(),
            "no source mutation performed".to_string(),
            "auto_promote=false".to_string(),
            "operator approval required".to_string(),
            "rollback manifest generated".to_string(),
            "network disabled".to_string(),
        ],
        created_at: context.generated_at,
    };
    write_release_bundle(memory_root, &bundle, &manifest, &rollback, &preflight)?;
    Ok(bundle)
}

pub fn print_release_bundle_json(
    project_root: &str,
    memory_root: &str,
    run_id: &str,
) -> Result<String, String> {
    let bundle = build_release_bundle(project_root, memory_root, run_id)?;
    serde_json::to_string_pretty(&bundle)
        .map_err(|error| format!("failed to serialize release bundle: {error}"))
}

pub fn print_release_manifest(memory_root: &str, release_id: &str) -> Result<String, String> {
    let path = release_paths(memory_root, release_id).manifest_path;
    fs::read_to_string(path).map_err(|error| format!("failed to read release manifest: {error}"))
}

pub fn print_release_changelog(memory_root: &str, release_id: &str) -> Result<String, String> {
    let path = release_paths(memory_root, release_id).changelog_path;
    fs::read_to_string(path).map_err(|error| format!("failed to read release changelog: {error}"))
}

pub fn print_rollback_manifest(memory_root: &str, release_id: &str) -> Result<String, String> {
    let path = release_paths(memory_root, release_id).rollback_path;
    fs::read_to_string(path).map_err(|error| format!("failed to read rollback manifest: {error}"))
}

pub fn list_releases(memory_root: &str) -> Result<Vec<String>, String> {
    let mut entries = load_release_entries(memory_root)?;
    entries.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.release_id.cmp(&right.release_id))
    });
    Ok(entries.into_iter().map(|entry| entry.release_id).collect())
}

pub fn release_count(memory_root: &str) -> Result<usize, String> {
    Ok(load_release_entries(memory_root)?.len())
}

pub fn latest_release_id(memory_root: &str) -> Result<Option<String>, String> {
    let mut entries = load_release_entries(memory_root)?;
    entries.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.release_id.cmp(&right.release_id))
    });
    Ok(entries.pop().map(|entry| entry.release_id))
}

pub fn print_release_status(memory_root: &str) -> Result<String, String> {
    let count = release_count(memory_root)?;
    let latest = latest_release_id(memory_root)?.unwrap_or_else(|| "none".to_string());
    Ok(format!(
        "releases={} latest={} auto_promote=false approval_required=true",
        count, latest
    ))
}

pub fn print_last_release(memory_root: &str) -> Result<String, String> {
    let entry = latest_release_entry(memory_root)?
        .ok_or_else(|| "no release bundles available".to_string())?;
    let bundle = load_release_bundle(memory_root, &entry.release_id)?;
    let preflight = load_release_preflight(memory_root, &entry.release_id)?;
    let manifest = load_release_manifest(memory_root, &entry.release_id)?;
    let rollback = load_rollback_manifest(memory_root, &entry.release_id)?;
    Ok(format!(
        "# Release Runtime EVA\n\nrelease_status: {}\n\n{}\n",
        print_release_status(memory_root)?,
        render_release_changelog(&bundle, &preflight, &manifest, &rollback)
    ))
}

pub(crate) struct ReleasePaths {
    pub bundle_path: String,
    pub manifest_path: String,
    pub rollback_path: String,
    pub changelog_path: String,
    pub preflight_report_path: String,
}

#[derive(Debug, Clone)]
struct ReleaseEntry {
    release_id: String,
    created_at: u64,
}

fn approval_status(context: &ReleaseCandidateContext) -> String {
    context
        .approval_record
        .as_ref()
        .map(|record| record.decision.clone())
        .unwrap_or_else(|| "missing".to_string())
}

fn write_release_bundle(
    memory_root: &str,
    bundle: &ReleaseBundle,
    manifest: &ReleaseManifest,
    rollback: &RollbackManifest,
    preflight: &ReleasePreflightReport,
) -> Result<(), String> {
    let paths = release_paths(memory_root, &bundle.release_id);
    let changelog = render_release_changelog(bundle, preflight, manifest, rollback);
    fs::create_dir_all(Path::new(memory_root).join("releases").join("bundles"))
        .map_err(|error| format!("failed to create release bundle dir: {error}"))?;
    fs::create_dir_all(Path::new(memory_root).join("releases").join("manifests"))
        .map_err(|error| format!("failed to create release manifest dir: {error}"))?;
    fs::create_dir_all(Path::new(memory_root).join("releases").join("rollback"))
        .map_err(|error| format!("failed to create rollback dir: {error}"))?;
    fs::create_dir_all(Path::new(memory_root).join("releases").join("changelogs"))
        .map_err(|error| format!("failed to create changelog dir: {error}"))?;
    fs::create_dir_all(Path::new(memory_root).join("releases").join("preflight"))
        .map_err(|error| format!("failed to create preflight dir: {error}"))?;
    memory::write_json(&paths.bundle_path, bundle)?;
    memory::write_json(&paths.manifest_path, manifest)?;
    memory::write_json(&paths.rollback_path, rollback)?;
    fs::write(&paths.changelog_path, changelog)
        .map_err(|error| format!("failed to write release changelog: {error}"))?;
    memory::write_json(&paths.preflight_report_path, preflight)
}

fn load_release_bundle(memory_root: &str, release_id: &str) -> Result<ReleaseBundle, String> {
    let path = release_paths(memory_root, release_id).bundle_path;
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read release bundle: {error}"))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("failed to parse release bundle: {error}"))
}

fn load_release_manifest(memory_root: &str, release_id: &str) -> Result<ReleaseManifest, String> {
    let path = release_paths(memory_root, release_id).manifest_path;
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read release manifest: {error}"))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("failed to parse release manifest: {error}"))
}

fn load_rollback_manifest(memory_root: &str, release_id: &str) -> Result<RollbackManifest, String> {
    let path = release_paths(memory_root, release_id).rollback_path;
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read rollback manifest: {error}"))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("failed to parse rollback manifest: {error}"))
}

fn load_release_preflight(
    memory_root: &str,
    release_id: &str,
) -> Result<ReleasePreflightReport, String> {
    let path = release_paths(memory_root, release_id).preflight_report_path;
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read preflight report: {error}"))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("failed to parse preflight report: {error}"))
}

fn latest_release_entry(memory_root: &str) -> Result<Option<ReleaseEntry>, String> {
    let mut entries = load_release_entries(memory_root)?;
    entries.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.release_id.cmp(&right.release_id))
    });
    Ok(entries.pop())
}

fn load_release_entries(memory_root: &str) -> Result<Vec<ReleaseEntry>, String> {
    let dir = Path::new(memory_root).join("releases").join("bundles");
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    for entry in
        fs::read_dir(&dir).map_err(|error| format!("failed to read release bundles: {error}"))?
    {
        let entry =
            entry.map_err(|error| format!("failed to read release bundle entry: {error}"))?;
        let path = entry.path();
        if !path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".json"))
        {
            continue;
        }
        let contents = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read release bundle: {error}"))?;
        let bundle: ReleaseBundle = serde_json::from_str(&contents)
            .map_err(|error| format!("failed to parse release bundle: {error}"))?;
        let created_at = if bundle.created_at == 0 {
            fs::metadata(&path)
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs())
                .unwrap_or(0)
        } else {
            bundle.created_at
        };
        let release_id = bundle.release_id.clone();
        entries.push(ReleaseEntry {
            release_id: release_id.clone(),
            created_at,
        });
    }
    entries.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.release_id.cmp(&right.release_id))
    });
    Ok(entries)
}

fn release_paths(memory_root: &str, release_id: &str) -> ReleasePaths {
    let root = Path::new(memory_root).join("releases");
    ReleasePaths {
        bundle_path: root
            .join("bundles")
            .join(format!("{release_id}.json"))
            .display()
            .to_string(),
        manifest_path: root
            .join("manifests")
            .join(format!("{release_id}.manifest.json"))
            .display()
            .to_string(),
        rollback_path: root
            .join("rollback")
            .join(format!("{release_id}.rollback.json"))
            .display()
            .to_string(),
        changelog_path: root
            .join("changelogs")
            .join(format!("{release_id}.ru.md"))
            .display()
            .to_string(),
        preflight_report_path: root
            .join("preflight")
            .join(format!("{release_id}.preflight.json"))
            .display()
            .to_string(),
    }
}
