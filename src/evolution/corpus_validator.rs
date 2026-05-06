use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::contracts::CorpusIngestContract;

const REQUIRED_DENIED_DIRS: [&str; 6] = [
    ".git",
    "target",
    "node_modules",
    "memory",
    "sandboxes",
    "eva_output",
];
const ALLOWED_EXTENSIONS: [&str; 4] = ["rs", "toml", "md", "json"];

pub fn validate_corpus_contract(contract: &CorpusIngestContract) -> Result<(), String> {
    if contract.max_files > 500 {
        return Err("max_files must be <= 500".to_string());
    }
    if contract.max_file_bytes > 262_144 {
        return Err("max_file_bytes must be <= 262144".to_string());
    }
    if contract.root_path.starts_with("http://") || contract.root_path.starts_with("https://") {
        return Err("network URLs are forbidden for corpus ingestion".to_string());
    }
    for extension in &contract.allowed_extensions {
        if !ALLOWED_EXTENSIONS.contains(&extension.as_str()) {
            return Err(format!("unsupported corpus extension: {extension}"));
        }
    }
    for denied in REQUIRED_DENIED_DIRS {
        if !contract.denied_dirs.iter().any(|value| value == denied) {
            return Err(format!("missing denied dir: {denied}"));
        }
    }
    let root = Path::new(&contract.root_path);
    if !root.exists() {
        return Err("corpus root_path must exist".to_string());
    }
    if !root.is_dir() {
        return Err("corpus root_path must be a directory".to_string());
    }
    reject_unsafe_absolute_root(root)?;
    if fs::symlink_metadata(root)
        .map_err(|error| format!("failed to inspect corpus root: {error}"))?
        .file_type()
        .is_symlink()
    {
        return Err("symlink corpus roots are forbidden".to_string());
    }
    Ok(())
}

pub fn validate_corpus_path(root: &Path, path: &Path, max_file_bytes: usize) -> Result<(), String> {
    let root_canonical = root
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize corpus root: {error}"))?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect corpus path: {error}"))?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "symlink traversal is forbidden: {}",
            path.display()
        ));
    }
    let canonical = path
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize corpus path: {error}"))?;
    if !canonical.starts_with(&root_canonical) {
        return Err(format!("corpus path escapes root: {}", path.display()));
    }
    if metadata.is_file() && metadata.len() > max_file_bytes as u64 {
        return Err(format!(
            "corpus file exceeds max_file_bytes: {}",
            path.display()
        ));
    }
    Ok(())
}

pub fn is_denied_path(path: &Path, denied_dirs: &[String]) -> bool {
    path.components().any(|component| match component {
        Component::Normal(name) => {
            let value = name.to_string_lossy();
            denied_dirs.iter().any(|denied| denied == &value)
        }
        _ => false,
    })
}

pub fn allowed_corpus_file(path: &Path, allowed_extensions: &[String]) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| allowed_extensions.iter().any(|allowed| allowed == ext))
}

fn reject_unsafe_absolute_root(root: &Path) -> Result<(), String> {
    if !root.is_absolute() {
        return Ok(());
    }
    let canonical = root
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize absolute corpus root: {error}"))?;
    let components = canonical
        .components()
        .filter(|component| matches!(component, Component::Normal(_)))
        .count();
    let banned = [
        PathBuf::from("/"),
        PathBuf::from("/etc"),
        PathBuf::from("/usr"),
        PathBuf::from("/home"),
    ];
    if banned.iter().any(|banned_root| &canonical == banned_root) || components < 3 {
        return Err(format!(
            "absolute unsafe corpus root is forbidden unless narrowed to a project folder: {}",
            root.display()
        ));
    }
    Ok(())
}
