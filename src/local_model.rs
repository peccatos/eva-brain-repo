use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

pub const DEFAULT_MODEL_URL: &str = "http://127.0.0.1:1234/v1/chat/completions";
pub const DEFAULT_MODEL_NAME: &str = "local-model";
pub const DEFAULT_MODEL_ID: &str = "eva-lite";
pub const BUILTIN_MODEL_ENDPOINT: &str = "builtin://eva-lite";
pub const BUILTIN_MODEL_NAME: &str = "eva-lite";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenAiModelConfig {
    pub id: String,
    pub endpoint: String,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_model_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    pub timeout_secs: u64,
}

impl Default for OpenAiModelConfig {
    fn default() -> Self {
        Self {
            id: DEFAULT_MODEL_ID.to_string(),
            endpoint: BUILTIN_MODEL_ENDPOINT.to_string(),
            model: BUILTIN_MODEL_NAME.to_string(),
            local_model_path: None,
            api_key: None,
            timeout_secs: 30,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelChatMessage {
    pub role: String,
    pub content: String,
}

impl ModelChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ModelChatOptions {
    pub temperature: f32,
    pub max_tokens: u32,
}

impl Default for ModelChatOptions {
    fn default() -> Self {
        Self {
            temperature: 0.2,
            max_tokens: 512,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelChatOutput {
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelHealth {
    pub reachable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    pub models: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OpenAiModelClient {
    config: OpenAiModelConfig,
}

impl OpenAiModelClient {
    pub fn new(config: OpenAiModelConfig) -> Result<Self, String> {
        if !is_builtin_endpoint(&config.endpoint) {
            validate_local_model_path(&config)?;
            parse_http_url(&config.endpoint)?;
        }
        Ok(Self { config })
    }

    pub fn config(&self) -> &OpenAiModelConfig {
        &self.config
    }

    pub fn chat(
        &self,
        messages: Vec<ModelChatMessage>,
        options: ModelChatOptions,
    ) -> Result<ModelChatOutput, String> {
        if is_builtin_endpoint(&self.config.endpoint) {
            return Ok(ModelChatOutput {
                content: builtin_advisory(&messages),
            });
        }

        let request = OpenAiChatRequest {
            model: self.config.model.clone(),
            messages,
            temperature: options.temperature,
            max_tokens: options.max_tokens,
        };

        let request_body = serde_json::to_string(&request)
            .map_err(|error| format!("failed to serialize model request: {error}"))?;
        let response = send_http_request(
            "POST",
            &self.config.endpoint,
            Some(&request_body),
            self.config.api_key.as_deref(),
            self.config.timeout_secs,
        )?;
        if !(200..300).contains(&response.status_code) {
            return Err(format!(
                "model request failed with HTTP {}: {}",
                response.status_code, response.body
            ));
        }

        parse_chat_response(&response.body).map(|content| ModelChatOutput { content })
    }

    pub fn health(&self) -> ModelHealth {
        if is_builtin_endpoint(&self.config.endpoint) {
            return ModelHealth {
                reachable: true,
                status: Some(200),
                models: vec![self.config.model.clone()],
                error: None,
            };
        }

        let Some(models_url) = models_url_from_chat_endpoint(&self.config.endpoint) else {
            return ModelHealth {
                reachable: false,
                status: None,
                models: Vec::new(),
                error: Some("model endpoint is not a /v1/chat/completions URL".to_string()),
            };
        };

        match send_http_request(
            "GET",
            &models_url,
            None,
            self.config.api_key.as_deref(),
            self.config.timeout_secs,
        ) {
            Ok(response) => {
                if !(200..300).contains(&response.status_code) {
                    return ModelHealth {
                        reachable: false,
                        status: Some(response.status_code),
                        models: Vec::new(),
                        error: Some(response.body),
                    };
                }
                match parse_models_response(&response.body) {
                    Ok(models) => ModelHealth {
                        reachable: true,
                        status: Some(response.status_code),
                        models,
                        error: None,
                    },
                    Err(error) => ModelHealth {
                        reachable: true,
                        status: Some(response.status_code),
                        models: Vec::new(),
                        error: Some(error),
                    },
                }
            }
            Err(error) => ModelHealth {
                reachable: false,
                status: None,
                models: Vec::new(),
                error: Some(error.to_string()),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
struct OpenAiChatRequest {
    model: String,
    messages: Vec<ModelChatMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoiceMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<OpenAiModelRecord>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModelRecord {
    id: String,
}

pub fn parse_chat_response(body: &str) -> Result<String, String> {
    let response: OpenAiChatResponse = serde_json::from_str(body)
        .map_err(|error| format!("failed to parse model response: {error}"))?;
    let content = response
        .choices
        .first()
        .and_then(|choice| choice.message.content.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "model response did not include choices[0].message.content".to_string())?;
    Ok(content.to_string())
}

pub fn parse_models_response(body: &str) -> Result<Vec<String>, String> {
    let response: OpenAiModelsResponse = serde_json::from_str(body)
        .map_err(|error| format!("failed to parse model list: {error}"))?;
    Ok(response.data.into_iter().map(|model| model.id).collect())
}

pub fn models_url_from_chat_endpoint(endpoint: &str) -> Option<String> {
    endpoint
        .trim_end_matches('/')
        .strip_suffix("/chat/completions")
        .map(|base| format!("{base}/models"))
}

pub fn is_builtin_endpoint(endpoint: &str) -> bool {
    endpoint == BUILTIN_MODEL_ENDPOINT
}

pub fn validate_local_model_path(config: &OpenAiModelConfig) -> Result<(), String> {
    let Some(path) = config
        .local_model_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
    else {
        return Ok(());
    };
    let metadata = Path::new(path).metadata().map_err(|error| {
        format!(
            "local model file for '{}' is not available at {}: {}",
            config.id, path, error
        )
    })?;
    if !metadata.is_file() {
        return Err(format!(
            "local model path for '{}' must be a file: {}",
            config.id, path
        ));
    }
    Ok(())
}

fn builtin_advisory(messages: &[ModelChatMessage]) -> String {
    let user_text = messages
        .iter()
        .rev()
        .find(|message| message.role == "user")
        .map(|message| message.content.as_str())
        .unwrap_or("");
    let lowered = user_text.to_ascii_lowercase();
    let mode = if lowered.contains("benchmark") {
        "benchmark"
    } else if lowered.contains("runtime") || lowered.contains("daemon") {
        "runtime"
    } else {
        "local"
    };
    format!(
        "eva-lite advisory: mode={mode}; keep execution local, use deterministic checks first, and call external models only by explicit model_id when extra reasoning is needed."
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SimpleHttpUrl {
    host: String,
    port: u16,
    path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SimpleHttpResponse {
    status_code: u16,
    body: String,
}

fn send_http_request(
    method: &str,
    url: &str,
    body: Option<&str>,
    api_key: Option<&str>,
    timeout_secs: u64,
) -> Result<SimpleHttpResponse, String> {
    let parsed = parse_http_url(url)?;
    let mut stream = TcpStream::connect((parsed.host.as_str(), parsed.port))
        .map_err(|error| format!("model connection failed for {url}: {error}"))?;
    let timeout = Duration::from_secs(timeout_secs.max(1));
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|error| format!("failed to set model read timeout: {error}"))?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|error| format!("failed to set model write timeout: {error}"))?;

    let body = body.unwrap_or("");
    let mut request = format!(
        "{method} {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nAccept: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
        parsed.path,
        parsed.host,
        body.as_bytes().len()
    );
    if let Some(api_key) = api_key.filter(|value| !value.trim().is_empty()) {
        request.push_str(&format!("Authorization: Bearer {api_key}\r\n"));
    }
    request.push_str("\r\n");
    request.push_str(body);

    stream
        .write_all(request.as_bytes())
        .map_err(|error| format!("failed to write model request: {error}"))?;

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|error| format!("failed to read model response: {error}"))?;
    parse_http_response(&response)
}

fn parse_http_url(url: &str) -> Result<SimpleHttpUrl, String> {
    let rest = url.strip_prefix("http://").ok_or_else(|| {
        "only http:// local model endpoints are supported without TLS deps".to_string()
    })?;
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    if authority.is_empty() {
        return Err("model URL is missing host".to_string());
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (
            host.to_string(),
            port.parse::<u16>()
                .map_err(|error| format!("invalid model URL port: {error}"))?,
        ),
        None => (authority.to_string(), 80),
    };
    if host.is_empty() {
        return Err("model URL is missing host".to_string());
    }
    Ok(SimpleHttpUrl {
        host,
        port,
        path: format!("/{path}"),
    })
}

fn parse_http_response(bytes: &[u8]) -> Result<SimpleHttpResponse, String> {
    let header_end = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| "model response missing HTTP headers".to_string())?;
    let headers = String::from_utf8_lossy(&bytes[..header_end]);
    let status_line = headers
        .lines()
        .next()
        .ok_or_else(|| "model response missing status line".to_string())?;
    let status_code = status_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| "model response missing status code".to_string())?
        .parse::<u16>()
        .map_err(|error| format!("invalid model response status code: {error}"))?;
    let body_bytes = &bytes[header_end + 4..];
    let body = if response_is_chunked(&headers) {
        String::from_utf8_lossy(&decode_chunked_body(body_bytes)?).to_string()
    } else {
        String::from_utf8_lossy(body_bytes).to_string()
    };
    Ok(SimpleHttpResponse { status_code, body })
}

fn response_is_chunked(headers: &str) -> bool {
    headers.lines().skip(1).any(|line| {
        line.split_once(':')
            .map(|(name, value)| {
                name.trim().eq_ignore_ascii_case("transfer-encoding")
                    && value
                        .split(',')
                        .any(|entry| entry.trim().eq_ignore_ascii_case("chunked"))
            })
            .unwrap_or(false)
    })
}

fn decode_chunked_body(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut index = 0;
    let mut decoded = Vec::new();

    loop {
        let line_end = find_crlf(bytes, index)
            .ok_or_else(|| "chunked model response missing chunk size".to_string())?;
        let size_line = String::from_utf8_lossy(&bytes[index..line_end]);
        let size_hex = size_line
            .split(';')
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "chunked model response has empty chunk size".to_string())?;
        let size = usize::from_str_radix(size_hex, 16)
            .map_err(|error| format!("invalid chunked model response size: {error}"))?;
        index = line_end + 2;
        if size == 0 {
            return Ok(decoded);
        }
        let chunk_end = index
            .checked_add(size)
            .ok_or_else(|| "chunked model response size overflow".to_string())?;
        if chunk_end + 2 > bytes.len() {
            return Err("chunked model response ended inside chunk".to_string());
        }
        decoded.extend_from_slice(&bytes[index..chunk_end]);
        if bytes.get(chunk_end..chunk_end + 2) != Some(b"\r\n") {
            return Err("chunked model response missing chunk terminator".to_string());
        }
        index = chunk_end + 2;
    }
}

fn find_crlf(bytes: &[u8], start: usize) -> Option<usize> {
    bytes
        .get(start..)?
        .windows(2)
        .position(|window| window == b"\r\n")
        .map(|position| start + position)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_chat_response_content() {
        let content = parse_chat_response(
            r#"{"choices":[{"message":{"role":"assistant","content":"  ready  "}}]}"#,
        )
        .expect("parse response");

        assert_eq!(content, "ready");
    }

    #[test]
    fn rejects_empty_choices() {
        let error = parse_chat_response(r#"{"choices":[]}"#).expect_err("empty choices");

        assert!(error.contains("choices[0].message.content"));
    }

    #[test]
    fn derives_models_url_from_chat_endpoint() {
        assert_eq!(
            models_url_from_chat_endpoint("http://127.0.0.1:1234/v1/chat/completions"),
            Some("http://127.0.0.1:1234/v1/models".to_string())
        );
    }

    #[test]
    fn parses_plain_http_model_url() {
        let url = parse_http_url("http://127.0.0.1:1234/v1/chat/completions").unwrap();

        assert_eq!(url.host, "127.0.0.1");
        assert_eq!(url.port, 1234);
        assert_eq!(url.path, "/v1/chat/completions");
    }

    #[test]
    fn rejects_https_without_tls_dependencies() {
        let error = parse_http_url("https://127.0.0.1:1234/v1/chat/completions")
            .expect_err("https unsupported");

        assert!(error.contains("http://"));
    }

    #[test]
    fn parses_chunked_http_model_response() {
        let response = parse_http_response(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n7\r\n{\"ok\":1\r\n2\r\n}\n\r\n0\r\n\r\n",
        )
        .expect("parse chunked");

        assert_eq!(response.status_code, 200);
        assert_eq!(response.body, "{\"ok\":1}\n");
    }

    #[test]
    fn builtin_model_returns_local_advisory_without_network() {
        let client = OpenAiModelClient::new(OpenAiModelConfig::default()).expect("builtin client");
        let output = client
            .chat(
                vec![ModelChatMessage::user("runtime daemon check")],
                ModelChatOptions::default(),
            )
            .expect("builtin chat");

        assert!(output.content.contains("eva-lite advisory"));
    }

    #[test]
    fn external_model_rejects_missing_local_model_path_before_network() {
        let config = OpenAiModelConfig {
            id: "fast".to_string(),
            endpoint: "http://127.0.0.1:1234/v1/chat/completions".to_string(),
            model: "tiny".to_string(),
            local_model_path: Some("/tmp/eva_missing_model.gguf".to_string()),
            api_key: None,
            timeout_secs: 30,
        };
        let error = OpenAiModelClient::new(config).expect_err("missing file should block backend");

        assert!(error.contains("local model file"));
    }
}
