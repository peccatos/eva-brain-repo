use std::fs;
use std::path::Path;

use crate::graph::ast_extract::extract_rust_ast;
use crate::graph::{load_graph, write_graph};

pub fn ingest_repo_patterns(repo_path: &str, memory_root: &str) -> Result<(), String> {
    let repo = Path::new(repo_path);
    if !repo.exists() {
        return Err("repo path does not exist".to_string());
    }
    let path = Path::new(memory_root).join("graph.json");
    let mut graph = load_graph(&path)?;
    ingest_cargo_dependencies(repo, &mut graph)?;
    ingest_rust_files(repo, repo, &mut graph)?;
    graph.compact();
    write_graph(&path, &graph)
}

fn ingest_cargo_dependencies(
    repo: &Path,
    graph: &mut crate::graph::EvolutionGraph,
) -> Result<(), String> {
    let cargo = repo.join("Cargo.toml");
    if !cargo.exists() {
        return Ok(());
    }
    let contents = fs::read_to_string(&cargo)
        .map_err(|error| format!("failed to read Cargo.toml: {error}"))?;
    let mut in_dependencies = false;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_dependencies = matches!(
                trimmed,
                "[dependencies]" | "[dev-dependencies]" | "[build-dependencies]"
            );
            continue;
        }
        if !in_dependencies || trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((name, _)) = trimmed.split_once('=') else {
            continue;
        };
        graph.upsert_node(&format!("dependency:{}", name.trim()), "Dependency");
    }
    Ok(())
}

fn ingest_rust_files(
    repo: &Path,
    current: &Path,
    graph: &mut crate::graph::EvolutionGraph,
) -> Result<(), String> {
    for entry in fs::read_dir(current).map_err(|error| format!("failed to read repo: {error}"))? {
        let entry = entry.map_err(|error| format!("failed to read repo entry: {error}"))?;
        let path = entry.path();
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if matches!(file_name, ".git" | "target" | "sandboxes" | "memory") {
            continue;
        }
        if entry
            .file_type()
            .map_err(|error| format!("failed to inspect repo entry: {error}"))?
            .is_dir()
        {
            ingest_rust_files(repo, &path, graph)?;
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }

        let relative = path
            .strip_prefix(repo)
            .map_err(|error| format!("failed to build repo relative path: {error}"))?
            .to_string_lossy()
            .to_string();
        let file = format!("file:{relative}");
        graph.upsert_node(&file, "File");
        let contents = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read Rust file: {error}"))?;
        let ast = extract_rust_ast(&contents)?;
        for module in ast.modules {
            upsert_pattern(graph, &format!("pattern:module:{module}"), &file);
        }
        for function in ast.functions {
            upsert_pattern(graph, &format!("pattern:function:{function}"), &file);
        }
        for item_struct in ast.structs {
            upsert_pattern(graph, &format!("pattern:struct:{item_struct}"), &file);
        }
        for item_enum in ast.enums {
            upsert_pattern(graph, &format!("pattern:enum:{item_enum}"), &file);
        }
        for import in ast.use_imports {
            upsert_pattern(graph, &format!("pattern:use:{import}"), &file);
        }
        for test in ast.test_functions {
            upsert_pattern(graph, &format!("pattern:test:{test}"), &file);
        }
    }
    Ok(())
}

fn upsert_pattern(graph: &mut crate::graph::EvolutionGraph, pattern: &str, file: &str) {
    graph.upsert_node(pattern, "Pattern");
    graph.upsert_edge(pattern, file, "found_in");
}
