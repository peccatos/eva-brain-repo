use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::contracts::EvolutionLogEntry;

pub mod analyzer;
pub mod ast_extract;
pub mod ingest;

pub const DEFAULT_GRAPH_PATH: &str = "memory/graph.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct EvolutionGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq, PartialOrd, Ord)]
pub struct GraphNode {
    pub id: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Eq, PartialOrd, Ord)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub relation: String,
}

pub fn update_graph_for_evolution(
    memory_root: &str,
    entry: &EvolutionLogEntry,
) -> Result<(), String> {
    let path = Path::new(memory_root).join("graph.json");
    let mut graph = load_graph(&path)?;
    let mutation = format!("mutation:{}", entry.mutation_id);
    let target = format!("file:{}", entry.target_file);
    let status = format!("status:{:?}", entry.status).to_ascii_lowercase();
    let score_band = format!("score_band:{}", score_band(entry.score));

    graph.upsert_node(&mutation, "Mutation");
    graph.upsert_node(&target, "TargetFile");
    graph.upsert_node(&status, "Status");
    graph.upsert_node(&score_band, "ScoreBand");
    graph.upsert_edge(&mutation, &target, "targets");
    graph.upsert_edge(&mutation, &status, "resulted_in");
    graph.upsert_edge(&mutation, &score_band, "scored_as");
    if let Some(hypothesis_id) = &entry.hypothesis_id {
        graph.upsert_node(hypothesis_id, "Hypothesis");
        graph.upsert_edge(hypothesis_id, &mutation, "suggested_mutation");
        for pattern in &entry.recombined_source_patterns {
            graph.upsert_node(pattern, "Pattern");
            graph.upsert_edge(hypothesis_id, pattern, "source_pattern");
        }
        for risk in &entry.recombined_avoided_risks {
            let risk_node = format!("risk:{risk}");
            graph.upsert_node(&risk_node, "Risk");
            graph.upsert_edge(hypothesis_id, &risk_node, "avoided_risk");
        }
    }
    graph.compact();
    write_graph(&path, &graph)
}

pub fn ingest_repo_patterns(repo_path: &str, memory_root: &str) -> Result<(), String> {
    ingest::ingest_repo_patterns(repo_path, memory_root)
}

fn score_band(score: f32) -> &'static str {
    if score >= 7.0 {
        "high"
    } else if score >= 5.0 {
        "candidate"
    } else {
        "low"
    }
}

pub(crate) fn load_graph(path: &Path) -> Result<EvolutionGraph, String> {
    if !path.exists() {
        return Ok(EvolutionGraph::default());
    }
    let contents =
        fs::read_to_string(path).map_err(|error| format!("failed to read graph: {error}"))?;
    serde_json::from_str(&contents).map_err(|error| format!("failed to parse graph: {error}"))
}

pub(crate) fn write_graph(path: &Path, graph: &EvolutionGraph) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create graph directory: {error}"))?;
    }
    let contents = serde_json::to_string_pretty(graph)
        .map_err(|error| format!("failed to serialize graph: {error}"))?;
    fs::write(path, contents).map_err(|error| format!("failed to write graph: {error}"))
}

impl EvolutionGraph {
    pub(crate) fn upsert_node(&mut self, id: &str, kind: &str) {
        self.nodes.push(GraphNode {
            id: id.to_string(),
            kind: kind.to_string(),
        });
    }

    pub(crate) fn upsert_edge(&mut self, from: &str, to: &str, relation: &str) {
        self.edges.push(GraphEdge {
            from: from.to_string(),
            to: to.to_string(),
            relation: relation.to_string(),
        });
    }

    pub(crate) fn compact(&mut self) {
        let nodes = self.nodes.drain(..).collect::<BTreeSet<_>>();
        self.nodes = nodes.into_iter().collect();
        let edges = self.edges.drain(..).collect::<BTreeSet<_>>();
        self.edges = edges.into_iter().collect();
    }
}
