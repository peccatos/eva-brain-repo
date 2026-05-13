use std::fmt;
use std::path::{Component, Path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SafePathError {
    Empty,
    Absolute,
    Traversal,
    Forbidden(String),
    NotAllowed(String),
}

impl fmt::Display for SafePathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "empty_path"),
            Self::Absolute => write!(f, "absolute_path"),
            Self::Traversal => write!(f, "path_traversal"),
            Self::Forbidden(path) => write!(f, "forbidden_path:{path}"),
            Self::NotAllowed(path) => write!(f, "path_not_allowed:{path}"),
        }
    }
}

pub fn validate_patch_path(path: &str) -> Result<(), SafePathError> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(SafePathError::Empty);
    }
    let path_obj = Path::new(trimmed);
    if path_obj.is_absolute() || trimmed.starts_with('~') {
        return Err(SafePathError::Absolute);
    }
    if path_obj
        .components()
        .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(SafePathError::Traversal);
    }
    for forbidden in [
        ".git/",
        "target/",
        "memory/",
        "releases/",
        "sandboxes/",
        ".eva-runtime-tests/",
        ".eva-evolution-tests/",
        "/etc/",
    ] {
        if trimmed == forbidden.trim_end_matches('/') || trimmed.starts_with(forbidden) {
            return Err(SafePathError::Forbidden(trimmed.to_string()));
        }
    }
    if trimmed == "Cargo.lock" {
        return Err(SafePathError::Forbidden(trimmed.to_string()));
    }
    if trimmed == "README.md"
        || trimmed.starts_with("src/")
        || trimmed.starts_with("tests/")
        || trimmed.starts_with("docs/")
    {
        return Ok(());
    }
    Err(SafePathError::NotAllowed(trimmed.to_string()))
}
