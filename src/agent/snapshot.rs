use std::fs;
use std::path::Path;

use crate::agent::storage::{id, memory_path, save_json_pretty};
use crate::contracts::PatchOp;

#[derive(serde::Serialize)]
struct FileSnapshot {
    snapshot_id: String,
    files: Vec<SnapshotFile>,
}

#[derive(serde::Serialize)]
struct SnapshotFile {
    path: String,
    existed: bool,
    content: Option<String>,
}

pub fn create_snapshot(
    project_root: &str,
    memory_root: &str,
    ops: &[PatchOp],
) -> Result<(String, String), String> {
    let snapshot_id = id("snapshot");
    let mut files = Vec::new();
    for op in ops {
        let path = Path::new(project_root).join(&op.path);
        let existed = path.exists();
        let content = if existed {
            Some(
                fs::read_to_string(&path)
                    .map_err(|error| format!("snapshot read {}: {error}", path.display()))?,
            )
        } else {
            None
        };
        files.push(SnapshotFile {
            path: op.path.clone(),
            existed,
            content,
        });
    }
    let snapshot = FileSnapshot {
        snapshot_id: snapshot_id.clone(),
        files,
    };
    let snapshot_path = memory_path(
        memory_root,
        &["applies", &format!("{snapshot_id}.snapshot.json")],
    );
    save_json_pretty(&snapshot_path, &snapshot)?;
    let rollback_path = memory_path(
        memory_root,
        &["applies", &format!("{snapshot_id}.rollback.json")],
    );
    save_json_pretty(
        &rollback_path,
        &serde_json::json!({
            "snapshot_id": snapshot_id,
            "rollback_type": "manual_metadata",
            "notes": ["Restore file contents from snapshot if operator chooses rollback."]
        }),
    )?;
    Ok((
        snapshot_path.display().to_string(),
        rollback_path.display().to_string(),
    ))
}
