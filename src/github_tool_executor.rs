use std::cmp::Ordering;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

#[cfg(feature = "github-online")]
use serde::Deserialize;

use crate::{
    DiscoveryConfig, GithubRepositorySummary, GithubSearchFixture, RepositoryDiscoveryCase,
    RepositoryDiscoveryManifest,
};

#[derive(Debug, Clone, PartialEq)]
pub struct DiscoveredRepository {
    pub full_name: String,
    pub html_url: String,
    pub description: Option<String>,
    pub license: String,
    pub default_branch: String,
    pub repo_size_kb: u64,
    pub has_tests_or_ci: bool,
    pub search_score: f64,
}

#[derive(Debug, Clone)]
pub struct GithubToolExecutor;

#[cfg(feature = "github-online")]
#[derive(Debug, Deserialize)]
struct GithubSearchResponse {
    items: Vec<GithubRepositorySummary>,
}

impl GithubToolExecutor {
    pub fn new() -> Self {
        Self
    }

    pub fn search_repositories(
        &self,
        config: &DiscoveryConfig,
        fixture_path: Option<&Path>,
    ) -> Result<Vec<DiscoveredRepository>, String> {
        let items = if let Some(path) = fixture_path {
            let contents = fs::read_to_string(path)
                .map_err(|error| format!("failed to read fixture {}: {}", path.display(), error))?;
            let fixture: GithubSearchFixture =
                serde_json::from_str(&contents).map_err(|error| {
                    format!("failed to parse fixture {}: {}", path.display(), error)
                })?;
            fixture.items
        } else {
            self.fetch_repositories(config)?
        };

        Ok(filter_and_score(items, config))
    }

    pub fn build_manifest(
        &self,
        repositories: Vec<DiscoveredRepository>,
        max_results: usize,
    ) -> RepositoryDiscoveryManifest {
        let cases = repositories
            .into_iter()
            .take(max_results)
            .enumerate()
            .map(|(index, repo)| RepositoryDiscoveryCase {
                case_id: make_case_id(index, &repo.full_name),
                repo_full_name: repo.full_name.clone(),
                repo_url: repo.html_url.clone(),
                license: repo.license.clone(),
                default_branch: repo.default_branch.clone(),
                source_type: "github_search".to_string(),
                source_reference: "repository_discovery".to_string(),
                goal: format!("подготовить benchmark-кейс для {}", repo.full_name),
                local_repo_path: format!("benchmarks/repos/{}", repo.full_name.replace('/', "_")),
                failure_type: "unknown".to_string(),
                initial_failure_observed: false,
                reproduction_notes: repo.description.clone(),
                repo_size_kb: Some(repo.repo_size_kb),
                has_tests_or_ci: repo.has_tests_or_ci,
                search_score: repo.search_score,
            })
            .collect::<Vec<_>>();

        RepositoryDiscoveryManifest { cases }
    }

    fn fetch_repositories(
        &self,
        config: &DiscoveryConfig,
    ) -> Result<Vec<GithubRepositorySummary>, String> {
        #[cfg(not(feature = "github-online"))]
        {
            let _ = config;
            return Err(
                "online GitHub discovery requires --features github-online or --fixture"
                    .to_string(),
            );
        }

        #[cfg(feature = "github-online")]
        {
            use reqwest::blocking::Client;
            use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT};

            let mut headers = HeaderMap::new();
            headers.insert(USER_AGENT, HeaderValue::from_static("eva-brain-repo/0.1"));
            if let Ok(token) = std::env::var("GITHUB_TOKEN") {
                let value = token.trim();
                if !value.is_empty() {
                    if let Ok(header) = HeaderValue::from_str(&format!("Bearer {value}")) {
                        headers.insert(AUTHORIZATION, header);
                    }
                }
            }

            let client = Client::builder()
                .default_headers(headers)
                .build()
                .map_err(|error| format!("failed to build github client: {error}"))?;
            let query = format!(
                "{} language:{} stars:>={}",
                config.query, config.language, config.min_stars
            );
            let response = client
                .get("https://api.github.com/search/repositories")
                .query(&[
                    ("q", query.as_str()),
                    ("sort", "stars"),
                    ("order", "desc"),
                    ("per_page", &config.max_results.to_string()),
                ])
                .send()
                .map_err(|error| format!("github search failed: {}", error))?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().unwrap_or_default();
                return Err(format!("github search failed: {} {}", status, body));
            }

            let parsed: GithubSearchResponse = response
                .json()
                .map_err(|error| format!("failed to parse github response: {}", error))?;
            Ok(parsed.items)
        }
    }
}

fn filter_and_score(
    raw_items: Vec<GithubRepositorySummary>,
    config: &DiscoveryConfig,
) -> Vec<DiscoveredRepository> {
    let mut seen = HashSet::new();
    let mut filtered = raw_items
        .into_iter()
        .filter(|repo| !repo.archived && !repo.disabled)
        .filter(|repo| repo.stargazers_count >= config.min_stars)
        .filter(|repo| full_name_allowed(&repo.full_name, &config.exclude_full_names))
        .filter(|repo| !should_exclude(&repo.full_name, &config.exclude_names))
        .filter(|repo| license_allowed(&normalize_license(repo), &config.license_allowlist))
        .filter(|repo| {
            config
                .min_repo_size_kb
                .map(|min| repo.size >= min)
                .unwrap_or(true)
        })
        .filter(|repo| {
            config
                .max_repo_size_kb
                .map(|max| repo.size <= max)
                .unwrap_or(true)
        })
        .filter(|repo| seen.insert(repo.full_name.clone()))
        .filter_map(|repo| {
            let has_tests_or_ci = heuristic_has_tests_or_ci(&repo);
            if config.require_tests_or_ci && !has_tests_or_ci {
                return None;
            }
            Some(DiscoveredRepository {
                full_name: repo.full_name.clone(),
                html_url: repo.html_url.clone(),
                description: repo.description.clone(),
                license: normalize_license(&repo),
                default_branch: repo.default_branch.clone(),
                repo_size_kb: repo.size,
                has_tests_or_ci,
                search_score: repository_score(&repo, config, has_tests_or_ci),
            })
        })
        .collect::<Vec<_>>();

    filtered.sort_by(|left, right| {
        right
            .search_score
            .partial_cmp(&left.search_score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| right.repo_size_kb.cmp(&left.repo_size_kb).reverse())
            .then_with(|| left.full_name.cmp(&right.full_name))
    });
    filtered
}

fn repository_score(
    repo: &GithubRepositorySummary,
    config: &DiscoveryConfig,
    has_tests_or_ci: bool,
) -> f64 {
    let size_penalty = ((repo.size as i64 - config.target_repo_size_kb as i64).abs() as f64)
        / config.target_repo_size_kb.max(1) as f64;
    let tests_bonus = if has_tests_or_ci { 1.25 } else { 0.0 };
    let issues_bonus = (repo.open_issues_count.min(25) as f64) / 25.0;
    let stars_bonus = (repo.stargazers_count.min(500) as f64) / 100.0;
    stars_bonus + tests_bonus + issues_bonus - size_penalty
}

fn heuristic_has_tests_or_ci(repo: &GithubRepositorySummary) -> bool {
    let haystack = format!(
        "{} {}",
        repo.full_name.to_ascii_lowercase(),
        repo.description
            .clone()
            .unwrap_or_default()
            .to_ascii_lowercase()
    );
    haystack.contains("test")
        || haystack.contains("ci")
        || haystack.contains("assert")
        || repo.open_issues_count > 0
        || repo.forks_count > 0
}

fn normalize_license(repo: &GithubRepositorySummary) -> String {
    repo.license
        .as_ref()
        .and_then(|license| license.spdx_id.clone())
        .unwrap_or_else(|| "UNKNOWN".to_string())
}

fn license_allowed(license: &str, allowlist: &[String]) -> bool {
    allowlist
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(license))
}

fn should_exclude(full_name: &str, exclude_names: &[String]) -> bool {
    let lowered = full_name.to_ascii_lowercase();
    exclude_names
        .iter()
        .any(|entry| lowered.contains(&entry.to_ascii_lowercase()))
}

fn full_name_allowed(full_name: &str, excluded: &[String]) -> bool {
    !excluded
        .iter()
        .any(|entry| full_name.eq_ignore_ascii_case(entry))
}

fn make_case_id(index: usize, full_name: &str) -> String {
    format!(
        "case_{:03}_{}",
        index + 1,
        full_name.replace('/', "_").replace('-', "_")
    )
}
