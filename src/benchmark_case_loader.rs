use std::fs;
use std::path::Path;

use crate::benchmark_contract::BenchmarkCaseManifest;

#[derive(Debug, Default, Clone, Copy)]
pub struct BenchmarkCaseLoader;

impl BenchmarkCaseLoader {
    pub fn load_manifest(path: impl AsRef<Path>) -> Result<BenchmarkCaseManifest, String> {
        let contents = fs::read_to_string(path.as_ref())
            .map_err(|error| format!("failed to read {}: {}", path.as_ref().display(), error))?;
        serde_json::from_str(&contents)
            .map_err(|error| format!("failed to parse {}: {}", path.as_ref().display(), error))
    }
}
