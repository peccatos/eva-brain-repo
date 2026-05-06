use std::fs;
use std::path::Path;

use eva_runtime_with_task_validator::{DiscoveryConfig, GithubToolExecutor};

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut config_path = "benchmarks/repo_discovery_config.json".to_string();
    let mut fixture_path = None;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--config" => {
                index += 1;
                config_path = args
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| config_path.clone());
            }
            "--fixture" => {
                index += 1;
                fixture_path = args.get(index).cloned();
            }
            unknown => {
                eprintln!("unsupported argument: {unknown}");
                std::process::exit(1);
            }
        }
        index += 1;
    }

    let result = run(&config_path, fixture_path.as_deref());
    match result {
        Ok(summary) => println!(
            "{}",
            serde_json::to_string_pretty(&summary).expect("serialize summary")
        ),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn run(config_path: &str, fixture_path: Option<&str>) -> Result<serde_json::Value, String> {
    let config_contents = fs::read_to_string(config_path)
        .map_err(|error| format!("failed to read config {}: {}", config_path, error))?;
    let config: DiscoveryConfig = serde_json::from_str(&config_contents)
        .map_err(|error| format!("failed to parse config {}: {}", config_path, error))?;

    let executor = GithubToolExecutor::new();
    let repositories = executor.search_repositories(&config, fixture_path.map(Path::new))?;
    let manifest = executor.build_manifest(repositories, config.max_results);
    if let Some(parent) = Path::new(&config.output_manifest_path).parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {}", parent.display(), error))?;
    }
    fs::write(
        &config.output_manifest_path,
        serde_json::to_string_pretty(&manifest).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("failed to write {}: {}", config.output_manifest_path, error))?;

    Ok(serde_json::json!({
        "status": "ok",
        "cases_written": manifest.cases.len(),
        "output": config.output_manifest_path,
    }))
}
