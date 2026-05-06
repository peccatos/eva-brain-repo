use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::benchmark_metrics::{BenchmarkAggregateMetrics, BenchmarkCaseMetrics};

pub const DEFAULT_BATCH_REPORT_PATH: &str = "benchmarks/rust_batch_report.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkBatchReport {
    pub generated_at: String,
    pub cases: Vec<BenchmarkCaseMetrics>,
    pub aggregate: BenchmarkAggregateMetrics,
}

impl BenchmarkBatchReport {
    pub fn new(cases: Vec<BenchmarkCaseMetrics>) -> Self {
        let aggregate = BenchmarkAggregateMetrics::from_cases(&cases);
        Self {
            generated_at: current_timestamp(),
            cases,
            aggregate,
        }
    }

    pub fn write_to_path(&self, path: impl AsRef<Path>) -> Result<(), String> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {}", parent.display(), error))?;
        }
        let contents = serde_json::to_string_pretty(self)
            .map_err(|error| format!("failed to serialize {}: {}", path.display(), error))?;
        fs::write(path, contents)
            .map_err(|error| format!("failed to write {}: {}", path.display(), error))
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path)
            .map_err(|error| format!("failed to read {}: {}", path.display(), error))?;
        serde_json::from_str(&contents)
            .map_err(|error| format!("failed to parse {}: {}", path.display(), error))
    }
}

fn current_timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("unix:{seconds}")
}
