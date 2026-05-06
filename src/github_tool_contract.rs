use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    pub language: String,
    pub query: String,
    pub license_allowlist: Vec<String>,
    #[serde(default)]
    pub exclude_full_names: Vec<String>,
    #[serde(default)]
    pub exclude_names: Vec<String>,
    #[serde(default)]
    pub min_repo_size_kb: Option<u64>,
    #[serde(default)]
    pub max_repo_size_kb: Option<u64>,
    #[serde(default = "default_target_repo_size_kb")]
    pub target_repo_size_kb: u64,
    #[serde(default = "default_true")]
    pub require_tests_or_ci: bool,
    pub min_stars: u64,
    pub max_results: usize,
    pub output_manifest_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GithubLicense {
    pub spdx_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GithubRepositorySummary {
    pub full_name: String,
    pub html_url: String,
    pub description: Option<String>,
    pub stargazers_count: u64,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub forks_count: u64,
    #[serde(default)]
    pub open_issues_count: u64,
    pub default_branch: String,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub disabled: bool,
    pub license: Option<GithubLicense>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GithubSearchFixture {
    pub items: Vec<GithubRepositorySummary>,
}

fn default_target_repo_size_kb() -> u64 {
    3_000
}
fn default_true() -> bool {
    true
}
