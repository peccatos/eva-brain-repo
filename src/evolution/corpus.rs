use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::contracts::CorpusIngestContract;
use crate::evolution::corpus_validator::{
    allowed_corpus_file, is_denied_path, validate_corpus_contract, validate_corpus_path,
};
use crate::graph::{load_graph, write_graph};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CorpusSummary {
    pub corpus_id: String,
    pub root_path: String,
    pub scanned_files: usize,
    pub skipped_files: usize,
    pub file_count: usize,
    pub rust_file_count: usize,
    pub test_file_count: usize,
    pub function_count: usize,
    pub test_function_count: usize,
    pub result_returning_functions: usize,
    pub error_enum_count: usize,
    pub validation_function_count: usize,
    pub cli_parser_mentions: usize,
    pub reporting_mentions: usize,
    pub module_names: Vec<String>,
    pub suggested_strategies: Vec<String>,
    pub safety_notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CorpusPatterns {
    pub corpus_id: String,
    pub detected_patterns: Vec<String>,
    pub symbolic_labels: Vec<String>,
}

pub fn default_corpus_contract(root_path: &str) -> CorpusIngestContract {
    CorpusIngestContract {
        corpus_id: format!(
            "corpus_{}",
            crate::contracts::sha256_digest(root_path)
                .chars()
                .take(8)
                .collect::<String>()
        ),
        root_path: root_path.to_string(),
        allowed_extensions: vec![
            "rs".to_string(),
            "toml".to_string(),
            "md".to_string(),
            "json".to_string(),
        ],
        allowed_dirs: vec![
            "src".to_string(),
            "tests".to_string(),
            "benches".to_string(),
            "examples".to_string(),
            ".".to_string(),
        ],
        denied_dirs: vec![
            ".git".to_string(),
            "target".to_string(),
            "node_modules".to_string(),
            "memory".to_string(),
            "sandboxes".to_string(),
            "eva_output".to_string(),
        ],
        max_files: 500,
        max_file_bytes: 262_144,
        extract_tests: true,
        extract_error_handling: true,
        extract_validation: true,
        extract_cli: true,
        extract_reporting: true,
        created_at: crate::evolution::memory::now_unix(),
    }
}

pub fn ingest_corpus(
    memory_root: &str,
    contract: &CorpusIngestContract,
) -> Result<CorpusSummary, String> {
    validate_corpus_contract(contract)?;
    let root = Path::new(&contract.root_path);
    let mut paths = Vec::new();
    let mut skipped_files = 0_usize;
    walk_corpus(root, root, contract, &mut paths, &mut skipped_files)?;

    let mut summary = CorpusSummary {
        corpus_id: contract.corpus_id.clone(),
        root_path: contract.root_path.clone(),
        scanned_files: paths.len(),
        skipped_files,
        safety_notes: vec![
            "read-only local ingestion only".to_string(),
            "source corpus was not mutated".to_string(),
            "no full source content stored".to_string(),
        ],
        ..CorpusSummary::default()
    };
    let mut detected = BTreeSet::new();
    let mut symbolic = BTreeSet::new();
    let mut modules = BTreeSet::new();

    for path in &paths {
        let relative = path
            .strip_prefix(root)
            .map_err(|error| format!("failed to build corpus relative path: {error}"))?
            .to_string_lossy()
            .to_string();
        summary.file_count += 1;
        if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            summary.rust_file_count += 1;
        }
        if relative.contains("test") {
            summary.test_file_count += 1;
        }
        let contents = fs::read_to_string(path)
            .map_err(|error| format!("failed to read corpus file: {error}"))?;
        extract_from_contents(
            &relative,
            &contents,
            &mut summary,
            &mut detected,
            &mut symbolic,
            &mut modules,
        );
    }

    summary.module_names = modules.into_iter().collect();
    summary.suggested_strategies = infer_strategies(&detected);
    let patterns = CorpusPatterns {
        corpus_id: contract.corpus_id.clone(),
        detected_patterns: detected.into_iter().collect(),
        symbolic_labels: symbolic.into_iter().collect(),
    };
    persist_corpus(memory_root, &summary, &patterns)?;
    update_graph_for_corpus(memory_root, &summary, &patterns)?;
    Ok(summary)
}

pub fn load_corpus_summary(memory_root: &str, corpus_id: &str) -> Result<CorpusSummary, String> {
    let path = Path::new(memory_root)
        .join("corpus")
        .join(format!("{corpus_id}.summary.json"));
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read corpus summary: {error}"))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("failed to parse corpus summary: {error}"))
}

pub fn load_corpus_patterns(memory_root: &str, corpus_id: &str) -> Result<CorpusPatterns, String> {
    let path = Path::new(memory_root)
        .join("corpus")
        .join(format!("{corpus_id}.patterns.json"));
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read corpus patterns: {error}"))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("failed to parse corpus patterns: {error}"))
}

pub fn list_corpora(memory_root: &str) -> Result<Vec<String>, String> {
    let dir = Path::new(memory_root).join("corpus");
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut ids = fs::read_dir(dir)
        .map_err(|error| format!("failed to read corpus dir: {error}"))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            entry
                .path()
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
        })
        .filter_map(|name| name.strip_suffix(".summary.json").map(str::to_string))
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    Ok(ids)
}

fn walk_corpus(
    root: &Path,
    current: &Path,
    contract: &CorpusIngestContract,
    paths: &mut Vec<PathBuf>,
    skipped_files: &mut usize,
) -> Result<(), String> {
    for entry in
        fs::read_dir(current).map_err(|error| format!("failed to read corpus dir: {error}"))?
    {
        let entry = entry.map_err(|error| format!("failed to read corpus entry: {error}"))?;
        let path = entry.path();
        if is_denied_path(&path, &contract.denied_dirs) {
            *skipped_files += 1;
            continue;
        }
        if validate_corpus_path(root, &path, contract.max_file_bytes).is_err() {
            *skipped_files += 1;
            continue;
        }
        let file_type = entry
            .file_type()
            .map_err(|error| format!("failed to inspect corpus entry: {error}"))?;
        if file_type.is_dir() {
            walk_corpus(root, &path, contract, paths, skipped_files)?;
            continue;
        }
        if !allowed_corpus_file(&path, &contract.allowed_extensions) {
            *skipped_files += 1;
            continue;
        }
        paths.push(path);
        if paths.len() >= contract.max_files {
            break;
        }
    }
    Ok(())
}

fn extract_from_contents(
    relative: &str,
    contents: &str,
    summary: &mut CorpusSummary,
    detected: &mut BTreeSet<String>,
    symbolic: &mut BTreeSet<String>,
    modules: &mut BTreeSet<String>,
) {
    if relative.ends_with(".rs") {
        for line in contents.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("fn ") || trimmed.contains(" fn ") {
                summary.function_count += 1;
            }
            if trimmed.starts_with("#[test]") {
                detected.insert("test_assertion_pattern".to_string());
            }
            if trimmed.contains("assert!") || trimmed.contains("assert_eq!") {
                summary.test_function_count += usize::from(relative.contains("test"));
                detected.insert("test_assertion_pattern".to_string());
                symbolic.insert("assertion_label".to_string());
            }
            if trimmed.contains("-> Result<") {
                summary.result_returning_functions += 1;
                detected.insert("result_error_pattern".to_string());
                symbolic.insert("result_signature_label".to_string());
            }
            if trimmed.contains("enum ") && trimmed.contains("Error") {
                summary.error_enum_count += 1;
                detected.insert("result_error_pattern".to_string());
                symbolic.insert("error_enum_label".to_string());
            }
            if trimmed.contains("validate") || trimmed.contains("guard") {
                summary.validation_function_count += 1;
                detected.insert("validation_guard_pattern".to_string());
                symbolic.insert("validation_guard_label".to_string());
            }
            if trimmed.contains("clap")
                || trimmed.contains("arg(")
                || trimmed.contains("subcommand")
            {
                summary.cli_parser_mentions += 1;
                detected.insert("cli_command_pattern".to_string());
                symbolic.insert("cli_parser_label".to_string());
            }
            if trimmed.contains("write_report")
                || trimmed.contains("report")
                || trimmed.contains("println!")
            {
                summary.reporting_mentions += 1;
                detected.insert("report_writer_pattern".to_string());
                symbolic.insert("report_writer_label".to_string());
            }
            if let Some(module) = trimmed.strip_prefix("mod ") {
                let name = module.trim_end_matches(';').trim().to_string();
                if !name.is_empty() {
                    modules.insert(name);
                }
            }
        }
        if contents.contains("serde_json") || contents.contains("json!") {
            detected.insert("json_contract_pattern".to_string());
            symbolic.insert("json_contract_label".to_string());
        }
    }
    if relative.ends_with(".toml") {
        detected.insert("toml_config_pattern".to_string());
        symbolic.insert("toml_config_label".to_string());
    }
}

fn infer_strategies(patterns: &BTreeSet<String>) -> Vec<String> {
    let mut strategies = BTreeSet::new();
    if patterns.contains("test_assertion_pattern") {
        strategies.insert("TestExpansion".to_string());
    }
    if patterns.contains("validation_guard_pattern") {
        strategies.insert("ValidationHardening".to_string());
    }
    if patterns.contains("report_writer_pattern") {
        strategies.insert("MetricsReporting".to_string());
    }
    if patterns.contains("result_error_pattern") {
        strategies.insert("RegressionAvoidance".to_string());
    }
    strategies.into_iter().collect()
}

fn persist_corpus(
    memory_root: &str,
    summary: &CorpusSummary,
    patterns: &CorpusPatterns,
) -> Result<(), String> {
    let dir = Path::new(memory_root).join("corpus");
    fs::create_dir_all(&dir).map_err(|error| format!("failed to create corpus dir: {error}"))?;
    crate::evolution::memory::write_json(
        dir.join(format!("{}.summary.json", summary.corpus_id)),
        summary,
    )?;
    crate::evolution::memory::write_json(
        dir.join(format!("{}.patterns.json", summary.corpus_id)),
        patterns,
    )?;
    fs::write(
        dir.join(format!("{}.ru.md", summary.corpus_id)),
        render_corpus_markdown(summary, patterns),
    )
    .map_err(|error| format!("failed to write corpus markdown: {error}"))
}

fn render_corpus_markdown(summary: &CorpusSummary, patterns: &CorpusPatterns) -> String {
    format!(
        "# Corpus EVA\n\ncorpus path: {}\nscanned files: {}\nskipped files: {}\ndetected patterns: {}\nsuggested strategies: {}\nsafety notes: {}\nsource corpus was not mutated: yes\n",
        summary.root_path,
        summary.scanned_files,
        summary.skipped_files,
        if patterns.detected_patterns.is_empty() {
            "(none)".to_string()
        } else {
            patterns.detected_patterns.join(", ")
        },
        if summary.suggested_strategies.is_empty() {
            "(none)".to_string()
        } else {
            summary.suggested_strategies.join(", ")
        },
        summary.safety_notes.join("; ")
    )
}

fn update_graph_for_corpus(
    memory_root: &str,
    summary: &CorpusSummary,
    patterns: &CorpusPatterns,
) -> Result<(), String> {
    let path = Path::new(memory_root).join("graph.json");
    let mut graph = load_graph(&path)?;
    let corpus_node = format!("corpus:{}", summary.corpus_id);
    graph.upsert_node(&corpus_node, "Corpus");
    for module in &summary.module_names {
        let module_node = format!("module:{module}");
        graph.upsert_node(&module_node, "Module");
        graph.upsert_edge(&corpus_node, &module_node, "corpus_mentions_module");
    }
    for pattern in &patterns.detected_patterns {
        let pattern_node = format!("corpus_pattern:{}:{}", summary.corpus_id, pattern);
        graph.upsert_node(&pattern_node, "CorpusPattern");
        graph.upsert_edge(&corpus_node, &pattern_node, "corpus_has_pattern");
    }
    for strategy in &summary.suggested_strategies {
        let strategy_node = format!("strategy:{strategy}");
        graph.upsert_node(&strategy_node, "Strategy");
        graph.upsert_edge(&corpus_node, &strategy_node, "corpus_suggests_strategy");
    }
    graph.compact();
    write_graph(&path, &graph)
}
