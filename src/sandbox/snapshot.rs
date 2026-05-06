use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

pub fn copy_project(project_root: &str, sandbox_path: &str) -> Result<(), String> {
    let source = Path::new(project_root);
    let destination = Path::new(sandbox_path);
    if !source.join("Cargo.toml").exists() {
        return Err("project root must contain Cargo.toml".to_string());
    }
    if destination.exists() {
        return Err("sandbox path already exists".to_string());
    }
    fs::create_dir_all(destination)
        .map_err(|error| format!("failed to create sandbox directory: {error}"))?;
    copy_dir_recursive(source, destination, source)
}

fn copy_dir_recursive(source: &Path, destination: &Path, root: &Path) -> Result<(), String> {
    for entry in
        fs::read_dir(source).map_err(|error| format!("failed to read directory: {error}"))?
    {
        let entry = entry.map_err(|error| format!("failed to read directory entry: {error}"))?;
        let source_path = entry.path();
        let relative_path = source_path
            .strip_prefix(root)
            .map_err(|error| format!("failed to build relative path: {error}"))?;
        if should_skip(relative_path) {
            continue;
        }

        let destination_path = destination.join(relative_path);
        if entry
            .file_type()
            .map_err(|error| format!("failed to inspect file type: {error}"))?
            .is_dir()
        {
            fs::create_dir_all(&destination_path)
                .map_err(|error| format!("failed to create sandbox subdirectory: {error}"))?;
            copy_dir_recursive(&source_path, destination, root)?;
        } else {
            if let Some(parent) = destination_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("failed to create sandbox file parent: {error}"))?;
            }
            fs::copy(&source_path, &destination_path)
                .map_err(|error| format!("failed to copy file {:?}: {error}", source_path))?;
        }
    }
    Ok(())
}

fn should_skip(relative_path: &Path) -> bool {
    let mut components = relative_path.components();
    let Some(first) = components.next() else {
        return false;
    };
    let first = PathBuf::from(first.as_os_str());
    matches!(
        first.as_os_str().to_str(),
        Some(".git" | "target" | "sandboxes" | "memory" | "eva_output")
    ) || relative_path
        .file_name()
        .is_some_and(|name| name == OsStr::new(".DS_Store"))
}
