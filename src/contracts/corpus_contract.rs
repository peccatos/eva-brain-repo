use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorpusIngestContract {
    pub corpus_id: String,
    pub root_path: String,
    pub allowed_extensions: Vec<String>,
    pub allowed_dirs: Vec<String>,
    pub denied_dirs: Vec<String>,
    pub max_files: usize,
    pub max_file_bytes: usize,
    pub extract_tests: bool,
    pub extract_error_handling: bool,
    pub extract_validation: bool,
    pub extract_cli: bool,
    pub extract_reporting: bool,
    pub created_at: u64,
}
