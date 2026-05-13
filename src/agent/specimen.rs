use std::fs;
use std::path::Path;

use crate::agent::storage::{memory_path, save_json_pretty};
use crate::contracts::SpecimenMetadata;

pub fn add_specimen(
    memory_root: &str,
    specimen_id: &str,
    path: &str,
) -> Result<SpecimenMetadata, String> {
    let metadata = SpecimenMetadata {
        specimen_id: specimen_id.into(),
        kind: "external_reference".into(),
        path: path.into(),
        allowed_use: "behavioral_reference_only".into(),
        source_copy_allowed: false,
        notes: Vec::new(),
    };
    save_json_pretty(
        &memory_path(
            memory_root,
            &["specimens", &format!("{specimen_id}.specimen.json")],
        ),
        &metadata,
    )?;
    Ok(metadata)
}

pub fn list_specimens(memory_root: &str) -> Result<Vec<SpecimenMetadata>, String> {
    let dir = Path::new(memory_root).join("specimens");
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut specimens = Vec::new();
    for entry in fs::read_dir(dir).map_err(|error| format!("read specimens: {error}"))? {
        let path = entry
            .map_err(|error| format!("read specimen entry: {error}"))?
            .path();
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.ends_with(".specimen.json"))
        {
            if let Ok(contents) = fs::read_to_string(&path) {
                if let Ok(metadata) = serde_json::from_str::<SpecimenMetadata>(&contents) {
                    specimens.push(metadata);
                }
            }
        }
    }
    specimens.sort_by(|a, b| a.specimen_id.cmp(&b.specimen_id));
    Ok(specimens)
}

pub fn print_specimen_add(
    memory_root: &str,
    specimen_id: &str,
    path: &str,
) -> Result<String, String> {
    let metadata = add_specimen(memory_root, specimen_id, path)?;
    Ok(format!(
        "specimen added\nspecimen_id={}\npath={}\nsource_copy_allowed=false",
        metadata.specimen_id, metadata.path
    ))
}

pub fn print_specimen_list(memory_root: &str) -> Result<String, String> {
    let specimens = list_specimens(memory_root)?;
    Ok(format!(
        "EVA Specimens\ncount={}\nids={}",
        specimens.len(),
        specimens
            .iter()
            .map(|s| s.specimen_id.as_str())
            .collect::<Vec<_>>()
            .join(",")
    ))
}
