use crate::{
    build_project_phase_runtime_output, CycleInput, ModelChatMessage, ModelChatOptions,
    OpenAiModelClient, OpenAiModelConfig, RuntimeCycleRunner, DEFAULT_MODEL_ID, DEFAULT_MODEL_NAME,
    DEFAULT_MODEL_URL,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub const DEFAULT_LISTEN_ADDR: &str = "127.0.0.1:8765";
pub const DEFAULT_RUNTIME_CONFIG_PATH: &str = "eva.runtime.json";
pub const RUNTIME_CLI_HELP: &str = r#"EVA runtime commands:
  cargo run
      Show this command list.

  cargo run -- --once
      Run one local deterministic runtime cycle and print JSON.

  cargo run -- --evolve
      Run one bounded self-evolution cycle in a disposable sandbox.

  cargo run -- --plan-evolution
      Print graph-guided plans without mutating or creating a sandbox.

  cargo run -- --evolve-planned
      Run one graph-guided bounded evolution cycle in a disposable sandbox.

  cargo run -- --evolve-planned-n <N>
      Run N graph-guided bounded evolution cycles in disposable sandboxes.

  cargo run -- --evolution-benchmark <N>
      Run N planned cycles and write aggregate benchmark reports.

  cargo run -- --autonomy-status
      Print current lightweight autonomy gate status.

  cargo run -- --metrics
      Print compact evolution metrics.

  cargo run -- --metrics-refresh
      Recompute compact evolution metrics from logs and memory files.

  cargo run -- --learning-summary
      Print compact learning memory summary.

  cargo run -- --last-report
      Print the latest Russian evolution report.

  cargo run -- --report <RUN_ID>
      Print a specific Russian evolution report.

  cargo run -- --report-refresh <RUN_ID>
      Rebuild a Russian evolution report from stored artifacts.

  cargo run -- --review-candidate <RUN_ID>
      Print candidate review with Russian summary and promotion readiness.

  cargo run -- --candidate-diff <RUN_ID>
      Print the bounded candidate payload or search/replace diff.

  cargo run -- --list-candidates
      List stored manual-promotion candidates.

  cargo run -- --replay <RUN_ID>
      Replay a stored candidate in a fresh sandbox.

  cargo run -- --promote <RUN_ID>
      Manually promote a gated candidate into the real project.

  cargo run -- --ingest-repo <PATH>
      Read local Rust repo patterns into memory/graph.json without mutating the repo.

  cargo run -- --run-task <PATH_TO_TASK_JSON>
      Validate, persist, and run a bounded evolution campaign from a task contract.

  cargo run -- --campaign <TASK_ID>
      Run a stored bounded evolution campaign by task id.

  cargo run -- --last-campaign-report
      Print the latest Russian campaign report.

  cargo run -- --distill-patterns
      Distill local-only successful and risky evolution patterns into memory/patterns/.

  cargo run -- --recombine-patterns
      Print top deterministic recombined hypotheses without mutation or sandbox creation.

  cargo run -- --evolve-recombined
      Run one recombined bounded evolution cycle in a disposable sandbox.

  cargo run -- --serve [--config eva.runtime.json]
      Start the HTTP runtime daemon. Defaults to 127.0.0.1:8765.

  cargo run -- --repo <REPO_URL>
      Run repo patch mode and write eva_output/report.md + summary.json.

  cargo run -- --serve --model-endpoint ID=MODEL@URL
      Add a local OpenAI-compatible model endpoint.

  cargo run -- --serve --model-file ID=/path/to/model.gguf --model-endpoint ID=MODEL@URL
      Guard an external local model endpoint by requiring a model file.

  cargo run --bin github_repo_discover -- --fixture fixtures/github_search_fixture.json
      Run offline benchmark discovery from the fixture.
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeCliCommand {
    Help,
    Once,
    Evolve,
    PlanEvolution,
    EvolvePlanned,
    EvolvePlannedN(usize),
    EvolutionBenchmark(usize),
    AutonomyStatus,
    Metrics,
    MetricsRefresh,
    LearningSummary,
    LastReport,
    Report(String),
    ReportRefresh(String),
    ReviewCandidate(String),
    CandidateDiff(String),
    ListCandidates,
    Replay(String),
    Promote(String),
    IngestRepo(String),
    RunTask(String),
    Campaign(String),
    LastCampaignReport,
    DistillPatterns,
    RecombinePatterns,
    EvolveRecombined,
    Serve(RuntimeDaemonConfig),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeDaemonConfig {
    pub listen_addr: String,
    pub default_model_id: String,
    pub models: Vec<OpenAiModelConfig>,
    pub managed_servers: Vec<ManagedServerConfig>,
}

impl Default for RuntimeDaemonConfig {
    fn default() -> Self {
        Self {
            listen_addr: DEFAULT_LISTEN_ADDR.to_string(),
            default_model_id: DEFAULT_MODEL_ID.to_string(),
            models: vec![OpenAiModelConfig::default()],
            managed_servers: Vec::new(),
        }
    }
}

fn parse_config_path(args: &[String]) -> Result<Option<String>, String> {
    let mut index = 0;
    let mut explicit_model_config = false;
    while index < args.len() {
        match args[index].as_str() {
            "--config" => {
                let Some(path) = args.get(index + 1) else {
                    return Err("--config requires a file path".to_string());
                };
                return Ok(Some(path.clone()));
            }
            value if value.starts_with("--config=") => {
                return Ok(Some(value.trim_start_matches("--config=").to_string()));
            }
            "--model-url" | "--model" | "--model-endpoint" | "--model-file" | "--start-server" => {
                explicit_model_config = true;
                index += 2;
            }
            value
                if value.starts_with("--model-url=")
                    || value.starts_with("--model=")
                    || value.starts_with("--model-endpoint=")
                    || value.starts_with("--model-file=")
                    || value.starts_with("--start-server=") =>
            {
                explicit_model_config = true;
                index += 1;
            }
            _ => index += 1,
        }
    }

    if let Ok(path) = std::env::var("EVA_RUNTIME_CONFIG") {
        if !path.trim().is_empty() {
            return Ok(Some(path));
        }
    }
    if !explicit_model_config && Path::new(DEFAULT_RUNTIME_CONFIG_PATH).exists() {
        return Ok(Some(DEFAULT_RUNTIME_CONFIG_PATH.to_string()));
    }
    Ok(None)
}

fn load_optional_runtime_config(path: Option<&str>) -> Result<Option<RuntimeDaemonConfig>, String> {
    let Some(path) = path else {
        return Ok(None);
    };
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("failed to read runtime config {}: {}", path, error))?;
    let config = serde_json::from_str::<RuntimeDaemonConfig>(&contents)
        .map_err(|error| format!("failed to parse runtime config {}: {}", path, error))?;
    Ok(Some(config))
}

impl RuntimeCliCommand {
    pub fn parse_from_iter<I, S>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let raw_args = args.into_iter().map(Into::into).collect::<Vec<_>>();
        if raw_args.is_empty()
            || raw_args
                .iter()
                .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
        {
            return Ok(Self::Help);
        }
        if raw_args == ["--once"] {
            return Ok(Self::Once);
        }
        if raw_args == ["--evolve"] {
            return Ok(Self::Evolve);
        }
        if raw_args == ["--plan-evolution"] {
            return Ok(Self::PlanEvolution);
        }
        if raw_args == ["--evolve-planned"] {
            return Ok(Self::EvolvePlanned);
        }
        if raw_args == ["--autonomy-status"] {
            return Ok(Self::AutonomyStatus);
        }
        if raw_args == ["--metrics"] {
            return Ok(Self::Metrics);
        }
        if raw_args == ["--metrics-refresh"] {
            return Ok(Self::MetricsRefresh);
        }
        if raw_args == ["--learning-summary"] {
            return Ok(Self::LearningSummary);
        }
        if raw_args == ["--last-report"] {
            return Ok(Self::LastReport);
        }
        if raw_args == ["--list-candidates"] {
            return Ok(Self::ListCandidates);
        }
        if raw_args == ["--last-campaign-report"] {
            return Ok(Self::LastCampaignReport);
        }
        if raw_args == ["--distill-patterns"] {
            return Ok(Self::DistillPatterns);
        }
        if raw_args == ["--recombine-patterns"] {
            return Ok(Self::RecombinePatterns);
        }
        if raw_args == ["--evolve-recombined"] {
            return Ok(Self::EvolveRecombined);
        }
        if raw_args.len() == 2 && raw_args[0] == "--report" {
            return Ok(Self::Report(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--report-refresh" {
            return Ok(Self::ReportRefresh(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--review-candidate" {
            return Ok(Self::ReviewCandidate(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--candidate-diff" {
            return Ok(Self::CandidateDiff(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--evolve-planned-n" {
            return Ok(Self::EvolvePlannedN(raw_args[1].parse::<usize>().map_err(
                |_| "--evolve-planned-n requires a positive integer".to_string(),
            )?));
        }
        if raw_args.len() == 2 && raw_args[0] == "--evolution-benchmark" {
            return Ok(Self::EvolutionBenchmark(
                raw_args[1]
                    .parse::<usize>()
                    .map_err(|_| "--evolution-benchmark requires a positive integer".to_string())?,
            ));
        }
        if raw_args.len() == 2 && raw_args[0] == "--replay" {
            return Ok(Self::Replay(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--promote" {
            return Ok(Self::Promote(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--ingest-repo" {
            return Ok(Self::IngestRepo(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--run-task" {
            return Ok(Self::RunTask(raw_args[1].clone()));
        }
        if raw_args.len() == 2 && raw_args[0] == "--campaign" {
            return Ok(Self::Campaign(raw_args[1].clone()));
        }

        let config_path = parse_config_path(&raw_args)?;
        let file_config = load_optional_runtime_config(config_path.as_deref())?;

        let mut serve = false;
        let mut once = false;
        let mut evolve = false;
        let mut listen_addr = file_config
            .as_ref()
            .map(|config| config.listen_addr.clone())
            .or_else(|| std::env::var("EVA_LISTEN_ADDR").ok())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_LISTEN_ADDR.to_string());
        let mut configured_default_model_id = file_config
            .as_ref()
            .map(|config| config.default_model_id.clone())
            .filter(|value| !value.trim().is_empty());
        let configured_models = file_config
            .as_ref()
            .map(|config| config.models.clone())
            .unwrap_or_default();
        let mut managed_servers = file_config
            .as_ref()
            .map(|config| config.managed_servers.clone())
            .unwrap_or_default();
        let env_model_endpoint = std::env::var("EVA_MODEL_URL")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let env_model_name = std::env::var("EVA_MODEL")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let mut model_endpoint = env_model_endpoint
            .clone()
            .unwrap_or_else(|| DEFAULT_MODEL_URL.to_string());
        let mut model_name = env_model_name
            .clone()
            .unwrap_or_else(|| DEFAULT_MODEL_NAME.to_string());
        let api_key = std::env::var("EVA_MODEL_API_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let base_model_file = std::env::var("EVA_MODEL_FILE")
            .ok()
            .filter(|value| !value.trim().is_empty());
        let mut model_specs = std::env::var("EVA_MODEL_ENDPOINTS")
            .ok()
            .map(|value| {
                value
                    .split(';')
                    .filter(|entry| !entry.trim().is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut model_file_specs = std::env::var("EVA_MODEL_FILES")
            .ok()
            .map(|value| {
                value
                    .split(';')
                    .filter(|entry| !entry.trim().is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mut server_specs = std::env::var("EVA_MODEL_SERVER_COMMANDS")
            .ok()
            .map(|value| {
                value
                    .split(';')
                    .filter(|entry| !entry.trim().is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut daemon_flag_used = false;
        let mut base_model_overridden = env_model_endpoint.is_some() || env_model_name.is_some();
        let mut args = raw_args.into_iter().peekable();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--help" | "-h" => return Ok(Self::Help),
                "--once" => once = true,
                "--evolve" => evolve = true,
                "--serve" => serve = true,
                "--config" => {
                    daemon_flag_used = true;
                    let _ = args
                        .next()
                        .ok_or_else(|| "--config requires a file path".to_string())?;
                }
                "--listen" => {
                    daemon_flag_used = true;
                    listen_addr = args
                        .next()
                        .ok_or_else(|| "--listen requires an address".to_string())?;
                }
                "--model-url" => {
                    daemon_flag_used = true;
                    base_model_overridden = true;
                    model_endpoint = args
                        .next()
                        .ok_or_else(|| "--model-url requires a URL".to_string())?;
                }
                "--model" => {
                    daemon_flag_used = true;
                    base_model_overridden = true;
                    model_name = args
                        .next()
                        .ok_or_else(|| "--model requires a model name".to_string())?;
                }
                "--model-endpoint" => {
                    daemon_flag_used = true;
                    model_specs.push(
                        args.next()
                            .ok_or_else(|| "--model-endpoint requires ID=MODEL@URL".to_string())?,
                    );
                }
                "--model-file" => {
                    daemon_flag_used = true;
                    model_file_specs.push(
                        args.next()
                            .ok_or_else(|| "--model-file requires ID=PATH".to_string())?,
                    );
                }
                "--start-server" => {
                    daemon_flag_used = true;
                    server_specs.push(
                        args.next()
                            .ok_or_else(|| "--start-server requires ID=COMMAND".to_string())?,
                    );
                }
                value if value.starts_with("--config=") => {
                    daemon_flag_used = true;
                }
                value if value.starts_with("--listen=") => {
                    daemon_flag_used = true;
                    listen_addr = value.trim_start_matches("--listen=").to_string();
                }
                value if value.starts_with("--model-url=") => {
                    daemon_flag_used = true;
                    base_model_overridden = true;
                    model_endpoint = value.trim_start_matches("--model-url=").to_string();
                }
                value if value.starts_with("--model=") => {
                    daemon_flag_used = true;
                    base_model_overridden = true;
                    model_name = value.trim_start_matches("--model=").to_string();
                }
                value if value.starts_with("--model-endpoint=") => {
                    daemon_flag_used = true;
                    model_specs.push(value.trim_start_matches("--model-endpoint=").to_string());
                }
                value if value.starts_with("--model-file=") => {
                    daemon_flag_used = true;
                    model_file_specs.push(value.trim_start_matches("--model-file=").to_string());
                }
                value if value.starts_with("--start-server=") => {
                    daemon_flag_used = true;
                    server_specs.push(value.trim_start_matches("--start-server=").to_string());
                }
                unknown => return Err(format!("unsupported runtime argument: {unknown}")),
            }
        }

        if once && serve {
            return Err("--once cannot be used with --serve".to_string());
        }
        if evolve && serve {
            return Err("--evolve cannot be used with --serve".to_string());
        }
        if once && evolve {
            return Err("--once cannot be used with --evolve".to_string());
        }

        if !serve {
            if daemon_flag_used {
                return Err("model daemon flags require --serve".to_string());
            }
            return Ok(if once {
                Self::Once
            } else if evolve {
                Self::Evolve
            } else {
                Self::Help
            });
        }

        if listen_addr.trim().is_empty() {
            return Err("--listen must not be empty".to_string());
        }
        if model_endpoint.trim().is_empty() {
            return Err("--model-url must not be empty".to_string());
        }
        if model_name.trim().is_empty() {
            return Err("--model must not be empty".to_string());
        }

        let base_model = OpenAiModelConfig {
            id: "default".to_string(),
            endpoint: model_endpoint,
            model: model_name,
            local_model_path: base_model_file,
            api_key: api_key.clone(),
            timeout_secs: 30,
        };
        let mut models = if !configured_models.is_empty() {
            configured_models
        } else if model_specs.is_empty() && !base_model_overridden {
            vec![OpenAiModelConfig::default()]
        } else if base_model_overridden {
            vec![base_model.clone()]
        } else {
            Vec::new()
        };
        if base_model_overridden && !models.iter().any(|model| model.id == "default") {
            models.insert(0, base_model);
        }
        for spec in model_specs {
            models.push(parse_model_endpoint_spec(&spec, api_key.clone())?);
        }
        apply_model_file_specs(&mut models, model_file_specs)?;
        if models.iter().all(|model| model.id != DEFAULT_MODEL_ID) {
            models.push(OpenAiModelConfig::default());
        }
        ensure_unique_model_ids(&models)?;
        let fallback_default_model_id = models
            .first()
            .map(|model| model.id.clone())
            .ok_or_else(|| "at least one model endpoint is required".to_string())?;
        for spec in server_specs {
            managed_servers.push(parse_managed_server_spec(&spec)?);
        }
        let default_model_id = configured_default_model_id
            .take()
            .filter(|model_id| models.iter().any(|model| model.id == *model_id))
            .unwrap_or(fallback_default_model_id);

        Ok(Self::Serve(RuntimeDaemonConfig {
            listen_addr,
            default_model_id,
            models,
            managed_servers,
        }))
    }
}

impl RuntimeDaemonConfig {
    pub fn default_model(&self) -> Result<&OpenAiModelConfig, String> {
        self.model_by_id(&self.default_model_id)
    }

    pub fn model_by_id(&self, model_id: &str) -> Result<&OpenAiModelConfig, String> {
        self.models
            .iter()
            .find(|model| model.id == model_id)
            .ok_or_else(|| format!("unknown model endpoint id: {model_id}"))
    }

    pub fn selected_model(&self, model_id: Option<&str>) -> Result<&OpenAiModelConfig, String> {
        match model_id {
            Some(value) if !value.trim().is_empty() => self.model_by_id(value),
            _ => self.default_model(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedServerConfig {
    pub id: String,
    pub command: String,
}

pub fn parse_model_endpoint_spec(
    spec: &str,
    api_key: Option<String>,
) -> Result<OpenAiModelConfig, String> {
    let (id, rest) = spec
        .split_once('=')
        .ok_or_else(|| "model endpoint must use ID=MODEL@URL".to_string())?;
    let (model, endpoint) = rest
        .split_once('@')
        .ok_or_else(|| "model endpoint must use ID=MODEL@URL".to_string())?;
    if id.trim().is_empty() || model.trim().is_empty() || endpoint.trim().is_empty() {
        return Err("model endpoint ID, MODEL, and URL must not be empty".to_string());
    }
    Ok(OpenAiModelConfig {
        id: id.trim().to_string(),
        endpoint: endpoint.trim().to_string(),
        model: model.trim().to_string(),
        local_model_path: None,
        api_key,
        timeout_secs: 30,
    })
}

pub fn apply_model_file_specs(
    models: &mut [OpenAiModelConfig],
    specs: Vec<String>,
) -> Result<(), String> {
    let model_ids = models
        .iter()
        .map(|model| model.id.clone())
        .collect::<std::collections::HashSet<_>>();
    for spec in specs {
        let (id, path) = spec
            .split_once('=')
            .ok_or_else(|| "model file must use ID=PATH".to_string())?;
        let id = id.trim();
        let path = path.trim();
        if id.is_empty() || path.is_empty() {
            return Err("model file ID and PATH must not be empty".to_string());
        }
        if !model_ids.contains(id) {
            return Err(format!(
                "model file references unknown model endpoint id: {id}"
            ));
        }
        if let Some(model) = models.iter_mut().find(|model| model.id == id) {
            model.local_model_path = Some(path.to_string());
        }
    }
    Ok(())
}

pub fn parse_managed_server_spec(spec: &str) -> Result<ManagedServerConfig, String> {
    let (id, command) = spec
        .split_once('=')
        .ok_or_else(|| "managed server must use ID=COMMAND".to_string())?;
    if id.trim().is_empty() || command.trim().is_empty() {
        return Err("managed server ID and COMMAND must not be empty".to_string());
    }
    Ok(ManagedServerConfig {
        id: id.trim().to_string(),
        command: command.trim().to_string(),
    })
}

struct ManagedServerChild {
    id: String,
    child: Child,
}

impl Drop for ManagedServerChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start_managed_servers(
    configs: &[ManagedServerConfig],
) -> Result<Vec<ManagedServerChild>, String> {
    configs
        .iter()
        .map(|config| {
            let parts = config.command.split_whitespace().collect::<Vec<_>>();
            let Some(program) = parts.first() else {
                return Err(format!("managed server {} has an empty command", config.id));
            };
            let child = Command::new(program)
                .args(&parts[1..])
                .stdin(Stdio::null())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()
                .map_err(|error| {
                    format!(
                        "failed to start managed server {} with '{}': {}",
                        config.id, config.command, error
                    )
                })?;
            Ok(ManagedServerChild {
                id: config.id.clone(),
                child,
            })
        })
        .collect()
}

fn ensure_unique_model_ids(models: &[OpenAiModelConfig]) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    for model in models {
        if !seen.insert(model.id.clone()) {
            return Err(format!("duplicate model endpoint id: {}", model.id));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeCycleHttpRequest {
    pub goal: String,
    pub context: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelChatHttpRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    #[serde(default)]
    pub messages: Vec<ModelChatMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeModelAdvisory {
    pub status: String,
    pub model_id: String,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeCycleHttpResponse {
    pub project_report_ru: crate::ProjectPhaseReport,
    pub runtime_audit: crate::RuntimeAudit,
    pub model_advisory: RuntimeModelAdvisory,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DaemonHealthResponse {
    pub daemon_status: String,
    pub listen_addr: String,
    pub default_model_id: String,
    pub managed_servers: Vec<ManagedServerConfig>,
    pub backends: Vec<ModelBackendHealth>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelRegistryResponse {
    pub default_model_id: String,
    pub models: Vec<OpenAiModelConfig>,
    pub managed_servers: Vec<ManagedServerConfig>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelBackendHealth {
    pub id: String,
    pub endpoint: String,
    pub model: String,
    pub backend: crate::ModelHealth,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status_code: u16,
    pub reason: &'static str,
    pub body: String,
}

pub fn serve(config: RuntimeDaemonConfig) -> Result<(), String> {
    let managed_children = start_managed_servers(&config.managed_servers)?;
    let listener = TcpListener::bind(&config.listen_addr)
        .map_err(|error| format!("failed to bind {}: {}", config.listen_addr, error))?;
    println!(
        "eva_runtime_daemon listening on {} with {} model endpoint(s), {} managed server(s); default={}",
        config.listen_addr,
        config.models.len(),
        managed_children.len(),
        config.default_model_id
    );
    for child in &managed_children {
        println!("managed model server started: {}", child.id);
    }

    let _managed_children = managed_children;
    let config = Arc::new(config);
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let config = Arc::clone(&config);
                thread::spawn(move || {
                    let mut stream = stream;
                    if let Err(error) = handle_stream(&mut stream, &config) {
                        eprintln!("daemon_request_error: {error}");
                    }
                });
            }
            Err(error) => eprintln!("daemon_accept_error: {error}"),
        }
    }

    Ok(())
}

pub fn handle_http_request(
    method: &str,
    path: &str,
    body: &str,
    config: &RuntimeDaemonConfig,
) -> HttpResponse {
    match (method, path) {
        ("GET", "/health") => json_response(
            200,
            "OK",
            &build_health_response(config).unwrap_or_else(|error| DaemonHealthResponse {
                daemon_status: "degraded".to_string(),
                listen_addr: config.listen_addr.clone(),
                default_model_id: config.default_model_id.clone(),
                managed_servers: config.managed_servers.clone(),
                backends: vec![ModelBackendHealth {
                    id: config.default_model_id.clone(),
                    endpoint: String::new(),
                    model: String::new(),
                    backend: crate::ModelHealth {
                        reachable: false,
                        status: None,
                        models: Vec::new(),
                        error: Some(error),
                    },
                }],
            }),
        ),
        ("GET", "/models") => json_response(
            200,
            "OK",
            &ModelRegistryResponse {
                default_model_id: config.default_model_id.clone(),
                models: config.models.clone(),
                managed_servers: config.managed_servers.clone(),
            },
        ),
        ("POST", "/runtime/cycle") => match serde_json::from_str::<RuntimeCycleHttpRequest>(body) {
            Ok(request) => match run_runtime_cycle_http(request, config) {
                Ok(response) => json_response(200, "OK", &response),
                Err(error) => error_response(500, "Internal Server Error", error),
            },
            Err(error) => error_response(
                400,
                "Bad Request",
                format!("invalid runtime request: {error}"),
            ),
        },
        ("POST", "/model/chat") => match serde_json::from_str::<ModelChatHttpRequest>(body) {
            Ok(request) => match run_model_chat_http(request, config) {
                Ok(response) => json_response(200, "OK", &response),
                Err(error) => error_response(500, "Internal Server Error", error),
            },
            Err(error) => error_response(
                400,
                "Bad Request",
                format!("invalid model request: {error}"),
            ),
        },
        _ => error_response(404, "Not Found", "route not found".to_string()),
    }
}

fn build_health_response(config: &RuntimeDaemonConfig) -> Result<DaemonHealthResponse, String> {
    let backends = config
        .models
        .iter()
        .map(|model| {
            let backend = OpenAiModelClient::new(model.clone())
                .map(|client| client.health())
                .unwrap_or_else(|error| crate::ModelHealth {
                    reachable: false,
                    status: None,
                    models: Vec::new(),
                    error: Some(error),
                });
            ModelBackendHealth {
                id: model.id.clone(),
                endpoint: model.endpoint.clone(),
                model: model.model.clone(),
                backend,
            }
        })
        .collect::<Vec<_>>();
    let any_reachable = backends.iter().any(|entry| entry.backend.reachable);
    Ok(DaemonHealthResponse {
        daemon_status: if any_reachable {
            "ok".to_string()
        } else {
            "degraded".to_string()
        },
        listen_addr: config.listen_addr.clone(),
        default_model_id: config.default_model_id.clone(),
        managed_servers: config.managed_servers.clone(),
        backends,
    })
}

fn run_runtime_cycle_http(
    request: RuntimeCycleHttpRequest,
    config: &RuntimeDaemonConfig,
) -> Result<RuntimeCycleHttpResponse, String> {
    let mut runner = RuntimeCycleRunner::new();
    let report = runner.run_cycle_report(CycleInput {
        goal: request.goal.clone(),
        external_state: request.context.clone(),
    })?;
    let output = build_project_phase_runtime_output(&report);
    let prompt = format!(
        "Goal: {}\nContext: {}\nRuntime audit JSON: {}\nReturn a concise operational recommendation.",
        request.goal,
        request.context,
        serde_json::to_string(&output.runtime_audit).expect("serialize audit")
    );
    let advisory = model_advisory(
        config,
        vec![
            ModelChatMessage::system(
                "You are a local runtime advisor. Be concrete, concise, and do not claim file mutations.",
            ),
            ModelChatMessage::user(prompt),
        ],
        request.model_id.as_deref(),
        ModelChatOptions::default(),
    )?;

    Ok(RuntimeCycleHttpResponse {
        project_report_ru: output.project_report_ru,
        runtime_audit: output.runtime_audit,
        model_advisory: advisory,
    })
}

fn run_model_chat_http(
    request: ModelChatHttpRequest,
    config: &RuntimeDaemonConfig,
) -> Result<RuntimeModelAdvisory, String> {
    let messages = if request.messages.is_empty() {
        vec![ModelChatMessage::user(
            request
                .prompt
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| "model request requires prompt or messages".to_string())?,
        )]
    } else {
        request.messages
    };
    let options = ModelChatOptions {
        temperature: request.temperature.unwrap_or(0.2),
        max_tokens: request.max_tokens.unwrap_or(512),
    };

    model_advisory(config, messages, request.model_id.as_deref(), options)
}

fn model_advisory(
    config: &RuntimeDaemonConfig,
    messages: Vec<ModelChatMessage>,
    model_id: Option<&str>,
    options: ModelChatOptions,
) -> Result<RuntimeModelAdvisory, String> {
    let selected = config.selected_model(model_id)?.clone();
    let client = match OpenAiModelClient::new(selected.clone()) {
        Ok(client) => client,
        Err(error) => {
            return Ok(RuntimeModelAdvisory {
                status: "error".to_string(),
                model_id: selected.id,
                model: selected.model,
                content: None,
                error: Some(error),
            });
        }
    };
    Ok(match client.chat(messages, options) {
        Ok(output) => RuntimeModelAdvisory {
            status: "ok".to_string(),
            model_id: client.config().id.clone(),
            model: client.config().model.clone(),
            content: Some(output.content),
            error: None,
        },
        Err(error) => RuntimeModelAdvisory {
            status: "error".to_string(),
            model_id: client.config().id.clone(),
            model: client.config().model.clone(),
            content: None,
            error: Some(error),
        },
    })
}

fn handle_stream(stream: &mut TcpStream, config: &RuntimeDaemonConfig) -> Result<(), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| format!("failed to set read timeout: {error}"))?;
    let request = read_http_request(stream)?;
    let response = handle_http_request(&request.method, &request.path, &request.body, config);
    write_http_response(stream, response)
}

struct HttpRequest {
    method: String,
    path: String,
    body: String,
}

fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 1024];
    let mut header_end = None;
    let mut content_length = 0_usize;

    loop {
        let bytes_read = stream
            .read(&mut chunk)
            .map_err(|error| format!("failed to read request: {error}"))?;
        if bytes_read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..bytes_read]);
        if buffer.len() > 1_048_576 {
            return Err("request exceeds 1 MiB limit".to_string());
        }
        if header_end.is_none() {
            if let Some(index) = find_header_end(&buffer) {
                header_end = Some(index);
                let headers = String::from_utf8_lossy(&buffer[..index]).to_string();
                content_length = parse_headers(&headers)
                    .remove("content-length")
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(0);
            }
        }
        if let Some(index) = header_end {
            if buffer.len() >= index + 4 + content_length {
                break;
            }
        }
    }

    let header_end = header_end.ok_or_else(|| "request missing HTTP headers".to_string())?;
    let headers = String::from_utf8_lossy(&buffer[..header_end]).to_string();
    let mut lines = headers.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| "request missing request line".to_string())?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| "request missing method".to_string())?
        .to_string();
    let path = parts
        .next()
        .ok_or_else(|| "request missing path".to_string())?
        .to_string();
    let body_start = header_end + 4;
    let body_end = body_start + content_length;
    let body =
        String::from_utf8_lossy(buffer.get(body_start..body_end).unwrap_or_default()).to_string();

    Ok(HttpRequest { method, path, body })
}

fn write_http_response(stream: &mut TcpStream, response: HttpResponse) -> Result<(), String> {
    let bytes = response.body.as_bytes();
    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response.status_code,
        response.reason,
        bytes.len()
    );
    stream
        .write_all(header.as_bytes())
        .and_then(|_| stream.write_all(bytes))
        .map_err(|error| format!("failed to write response: {error}"))
}

fn json_response<T: Serialize>(status_code: u16, reason: &'static str, value: &T) -> HttpResponse {
    HttpResponse {
        status_code,
        reason,
        body: serde_json::to_string_pretty(value).expect("serialize HTTP response"),
    }
}

fn error_response(status_code: u16, reason: &'static str, error: String) -> HttpResponse {
    json_response(status_code, reason, &serde_json::json!({ "error": error }))
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_headers(headers: &str) -> HashMap<String, String> {
    headers
        .lines()
        .skip(1)
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_ascii_lowercase(), value.trim().to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_cli_defaults_to_help() {
        assert_eq!(
            RuntimeCliCommand::parse_from_iter(Vec::<String>::new()).unwrap(),
            RuntimeCliCommand::Help
        );
        assert_eq!(
            RuntimeCliCommand::parse_from_iter(["--help"]).unwrap(),
            RuntimeCliCommand::Help
        );
    }

    #[test]
    fn runtime_cli_parses_once() {
        assert_eq!(
            RuntimeCliCommand::parse_from_iter(["--once"]).unwrap(),
            RuntimeCliCommand::Once
        );
    }

    #[test]
    fn runtime_cli_parses_evolve() {
        assert_eq!(
            RuntimeCliCommand::parse_from_iter(["--evolve"]).unwrap(),
            RuntimeCliCommand::Evolve
        );
    }

    #[test]
    fn runtime_cli_parses_serve_config() {
        let command = RuntimeCliCommand::parse_from_iter([
            "--serve",
            "--listen",
            "127.0.0.1:9999",
            "--model-url",
            "http://127.0.0.1:1234/v1/chat/completions",
            "--model",
            "demo",
        ])
        .expect("parse serve");

        match command {
            RuntimeCliCommand::Serve(config) => {
                assert_eq!(config.listen_addr, "127.0.0.1:9999");
                assert_eq!(config.default_model().unwrap().model, "demo");
                assert!(config.model_by_id(DEFAULT_MODEL_ID).is_ok());
            }
            RuntimeCliCommand::Help => panic!("expected serve command"),
            RuntimeCliCommand::Once => panic!("expected serve command"),
            RuntimeCliCommand::Evolve => panic!("expected serve command"),
            _ => panic!("expected serve command"),
        }
    }

    #[test]
    fn daemon_flags_require_serve() {
        let error =
            RuntimeCliCommand::parse_from_iter(["--model", "demo"]).expect_err("missing serve");

        assert!(error.contains("--serve"));
    }

    #[test]
    fn unknown_route_returns_404() {
        let config = RuntimeDaemonConfig::default();
        let response = handle_http_request("GET", "/missing", "", &config);

        assert_eq!(response.status_code, 404);
    }

    #[test]
    fn runtime_cli_parses_multiple_model_endpoints() {
        let command = RuntimeCliCommand::parse_from_iter([
            "--serve",
            "--model-endpoint",
            "fast=tiny@http://127.0.0.1:1234/v1/chat/completions",
            "--model-endpoint",
            "deep=larger@http://127.0.0.1:8080/v1/chat/completions",
        ])
        .expect("parse multi model");

        match command {
            RuntimeCliCommand::Serve(config) => {
                assert_eq!(config.default_model_id, "fast");
                assert_eq!(config.models.len(), 3);
                assert_eq!(config.model_by_id("deep").unwrap().model, "larger");
                assert_eq!(
                    config.model_by_id(DEFAULT_MODEL_ID).unwrap().model,
                    "eva-lite"
                );
            }
            RuntimeCliCommand::Help => panic!("expected serve command"),
            RuntimeCliCommand::Once => panic!("expected serve command"),
            RuntimeCliCommand::Evolve => panic!("expected serve command"),
            _ => panic!("expected serve command"),
        }
    }

    #[test]
    fn runtime_cli_attaches_local_model_file_guard() {
        let command = RuntimeCliCommand::parse_from_iter([
            "--serve",
            "--model-endpoint",
            "fast=tiny@http://127.0.0.1:1234/v1/chat/completions",
            "--model-file",
            "fast=/models/tiny.gguf",
        ])
        .expect("parse guarded model");

        match command {
            RuntimeCliCommand::Serve(config) => {
                assert_eq!(
                    config
                        .model_by_id("fast")
                        .unwrap()
                        .local_model_path
                        .as_deref(),
                    Some("/models/tiny.gguf")
                );
            }
            RuntimeCliCommand::Help => panic!("expected serve command"),
            RuntimeCliCommand::Once => panic!("expected serve command"),
            RuntimeCliCommand::Evolve => panic!("expected serve command"),
            _ => panic!("expected serve command"),
        }
    }

    #[test]
    fn guarded_external_model_reports_error_without_network() {
        let command = RuntimeCliCommand::parse_from_iter([
            "--serve",
            "--model-endpoint",
            "fast=tiny@http://127.0.0.1:1234/v1/chat/completions",
            "--model-file",
            "fast=/tmp/eva_missing_model.gguf",
        ])
        .expect("parse guarded model");
        let RuntimeCliCommand::Serve(config) = command else {
            panic!("expected serve command");
        };

        let response = handle_http_request(
            "POST",
            "/model/chat",
            r#"{"prompt":"check","model_id":"fast"}"#,
            &config,
        );
        let advisory =
            serde_json::from_str::<RuntimeModelAdvisory>(&response.body).expect("advisory json");

        assert_eq!(response.status_code, 200);
        assert_eq!(advisory.status, "error");
        assert!(advisory.error.unwrap().contains("local model file"));
    }

    #[test]
    fn runtime_cli_plain_serve_uses_local_runtime_config_when_present() {
        let command = RuntimeCliCommand::parse_from_iter(["--serve"]).expect("parse serve");

        match command {
            RuntimeCliCommand::Serve(config) => {
                assert_eq!(config.default_model_id, "qwen3-local");
                assert_eq!(config.default_model().unwrap().model, "qwen3:1.7b");
                assert!(config.model_by_id(DEFAULT_MODEL_ID).is_ok());
            }
            RuntimeCliCommand::Help => panic!("expected serve command"),
            RuntimeCliCommand::Once => panic!("expected serve command"),
            RuntimeCliCommand::Evolve => panic!("expected serve command"),
            _ => panic!("expected serve command"),
        }
    }

    #[test]
    fn runtime_cli_parses_managed_server_command() {
        let command = RuntimeCliCommand::parse_from_iter([
            "--serve",
            "--start-server",
            "lm=python3 /tmp/openai_mock.py",
        ])
        .expect("parse managed server");

        match command {
            RuntimeCliCommand::Serve(config) => {
                assert_eq!(config.managed_servers.len(), 1);
                assert_eq!(config.managed_servers[0].id, "lm");
                assert_eq!(
                    config.managed_servers[0].command,
                    "python3 /tmp/openai_mock.py"
                );
            }
            RuntimeCliCommand::Help => panic!("expected serve command"),
            RuntimeCliCommand::Once => panic!("expected serve command"),
            RuntimeCliCommand::Evolve => panic!("expected serve command"),
            _ => panic!("expected serve command"),
        }
    }

    #[test]
    fn runtime_cli_loads_json_config() {
        let path = std::env::temp_dir().join(format!(
            "eva_runtime_config_test_{}_{}.json",
            std::process::id(),
            "config"
        ));
        std::fs::write(
            &path,
            r#"{
  "listen_addr": "127.0.0.1:9998",
  "default_model_id": "eva-lite",
  "models": [
    {
      "id": "eva-lite",
      "endpoint": "builtin://eva-lite",
      "model": "eva-lite",
      "timeout_secs": 30
    }
  ],
  "managed_servers": []
}"#,
        )
        .expect("write config");

        let command = RuntimeCliCommand::parse_from_iter([
            "--serve".to_string(),
            "--config".to_string(),
            path.display().to_string(),
        ])
        .expect("parse config");

        match command {
            RuntimeCliCommand::Serve(config) => {
                assert_eq!(config.listen_addr, "127.0.0.1:9998");
                assert_eq!(config.default_model_id, DEFAULT_MODEL_ID);
            }
            RuntimeCliCommand::Help => panic!("expected serve command"),
            RuntimeCliCommand::Once => panic!("expected serve command"),
            RuntimeCliCommand::Evolve => panic!("expected serve command"),
            _ => panic!("expected serve command"),
        }

        let _ = std::fs::remove_file(path);
    }
}
