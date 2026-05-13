use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkspaceInspection {
    pub inspection_id: String,
    pub generated_at: u64,
    pub repo_root: String,
    pub git_status: String,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub language: String,
    pub cargo_project: bool,
    pub cargo_toml_exists: bool,
    pub lockfile_exists: bool,
    pub entrypoints: Vec<String>,
    pub source_dirs: Vec<String>,
    pub test_dirs: Vec<String>,
    pub docs_dirs: Vec<String>,
    pub available_commands: Vec<String>,
    pub risk_zones: Vec<String>,
    pub ignored_zones: Vec<String>,
    pub warnings: Vec<String>,
    pub blockers: Vec<String>,
}
