use crate::contracts::{LlmRequest, LlmResponse, LlmStatus};
use crate::llm::provider::LlmProvider;
use crate::llm::rule_based::RuleBasedLlmProvider;
use crate::llm::sanitize::sanitize_llm_context;
use crate::llm::schemas::schema_for;
use serde_json::{json, Value};

#[derive(Debug, Clone)]
pub struct OpenAiLlmProvider {
    pub model: String,
}

impl Default for OpenAiLlmProvider {
    fn default() -> Self {
        Self {
            model: std::env::var("EVE_OPENAI_MODEL").unwrap_or_else(|_| "gpt-5.5".to_string()),
        }
    }
}

impl LlmProvider for OpenAiLlmProvider {
    fn name(&self) -> &'static str {
        "openai"
    }

    fn complete(&self, request: &LlmRequest) -> Result<LlmResponse, String> {
        let sanitized = sanitize_llm_context(&request.input);
        if sanitized.blocked {
            return Ok(LlmResponse {
                request_id: request.request_id.clone(),
                provider: self.name().to_string(),
                model: self.model.clone(),
                status: LlmStatus::BlockedBySanitizer,
                output_text: String::new(),
                parsed_json: None,
                warnings: sanitized.redactions,
                blockers: sanitized.blockers,
            });
        }
        if std::env::var("OPENAI_API_KEY")
            .ok()
            .filter(|v| !v.is_empty())
            .is_none()
        {
            return Ok(LlmResponse {
                request_id: request.request_id.clone(),
                provider: self.name().to_string(),
                model: self.model.clone(),
                status: LlmStatus::NotConfigured,
                output_text: String::new(),
                parsed_json: None,
                warnings: vec!["openai_api_key_missing".to_string()],
                blockers: Vec::new(),
            });
        }
        let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
        let payload = responses_api_payload(request, &sanitized.text, &self.model);
        let response = reqwest::blocking::Client::new()
            .post("https://api.openai.com/v1/responses")
            .bearer_auth(api_key)
            .json(&payload)
            .send()
            .map_err(|error| format!("openai request failed: {error}"))?;
        let status = response.status();
        let body: Value = response
            .json()
            .map_err(|error| format!("openai response parse failed: {error}"))?;
        if !status.is_success() {
            return Ok(LlmResponse {
                request_id: request.request_id.clone(),
                provider: self.name().to_string(),
                model: self.model.clone(),
                status: LlmStatus::Failed,
                output_text: String::new(),
                parsed_json: Some(body),
                warnings: Vec::new(),
                blockers: vec![format!("openai_http_status:{status}")],
            });
        }
        let output_text = extract_output_text(&body);
        let parsed_json = serde_json::from_str::<Value>(&output_text).ok();
        Ok(LlmResponse {
            request_id: request.request_id.clone(),
            provider: self.name().to_string(),
            model: self.model.clone(),
            status: LlmStatus::Completed,
            output_text,
            parsed_json,
            warnings: Vec::new(),
            blockers: Vec::new(),
        })
    }
}

pub fn responses_api_payload(request: &LlmRequest, sanitized_input: &str, model: &str) -> Value {
    let mut payload = json!({
        "model": model,
        "instructions": request.system_prompt,
        "input": sanitized_input,
        "max_output_tokens": request.max_output_tokens
    });
    if request.temperature > 0.0 {
        payload["temperature"] = json!(request.temperature);
    }
    if !request.expected_schema.trim().is_empty() {
        payload["text"] = json!({
            "format": {
                "type": "json_schema",
                "name": request.expected_schema,
                "strict": true,
                "schema": schema_for(&request.expected_schema)
            }
        });
    }
    payload
}

fn extract_output_text(body: &Value) -> String {
    if let Some(text) = body.get("output_text").and_then(Value::as_str) {
        return text.to_string();
    }
    body.get("output")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|item| {
            item.get("content")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter_map(|content| content.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn openai_selected_from_env() -> bool {
    let configured = std::env::var("OPENAI_API_KEY")
        .ok()
        .filter(|v| !v.is_empty())
        .is_some();
    let mode = std::env::var("EVE_LLM_MODE").unwrap_or_else(|_| {
        if configured {
            "openai".to_string()
        } else {
            "rule_based".to_string()
        }
    });
    let provider_env = std::env::var("EVE_LLM_PROVIDER").unwrap_or_else(|_| {
        if configured {
            "openai".to_string()
        } else {
            "rule_based".to_string()
        }
    });
    configured && mode != "rule_based" && provider_env == "openai"
}

pub fn selected_llm_provider_name_from_env() -> &'static str {
    if openai_selected_from_env() {
        "openai"
    } else {
        "rule_based"
    }
}

pub fn select_llm_provider_from_env() -> Box<dyn LlmProvider> {
    if openai_selected_from_env() {
        Box::new(OpenAiLlmProvider::default())
    } else {
        Box::new(RuleBasedLlmProvider)
    }
}

pub fn llm_health() -> String {
    let configured = std::env::var("OPENAI_API_KEY")
        .ok()
        .filter(|v| !v.is_empty())
        .is_some();
    let model = std::env::var("EVE_OPENAI_MODEL").unwrap_or_else(|_| "gpt-5.5".to_string());
    let provider = selected_llm_provider_name_from_env();
    format!(
        "EVA LLM Health\nprovider={provider}\nopenai_configured={configured}\nmodel={model}\nfallback_available=true\nstatus={}\n",
        if configured || provider == "rule_based" { "ok" } else { "not_configured" }
    )
}
