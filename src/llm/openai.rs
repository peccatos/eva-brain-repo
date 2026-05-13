use crate::contracts::{LlmRequest, LlmResponse, LlmStatus};
use crate::llm::provider::LlmProvider;
use crate::llm::sanitize::sanitize_llm_context;

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
        Ok(LlmResponse {
            request_id: request.request_id.clone(),
            provider: self.name().to_string(),
            model: self.model.clone(),
            status: LlmStatus::Failed,
            output_text: String::new(),
            parsed_json: None,
            warnings: Vec::new(),
            blockers: vec!["openai_network_call_not_enabled_in_tests".to_string()],
        })
    }
}

pub fn llm_health() -> String {
    let configured = std::env::var("OPENAI_API_KEY")
        .ok()
        .filter(|v| !v.is_empty())
        .is_some();
    let mode = std::env::var("EVE_LLM_MODE").unwrap_or_else(|_| "rule_based".to_string());
    let model = std::env::var("EVE_OPENAI_MODEL").unwrap_or_else(|_| "gpt-5.5".to_string());
    let provider = if configured && mode == "openai" {
        "openai"
    } else {
        "rule_based"
    };
    format!(
        "EVA LLM Health\nprovider={provider}\nopenai_configured={configured}\nmodel={model}\nfallback_available=true\nstatus={}\n",
        if configured || provider == "rule_based" { "ok" } else { "not_configured" }
    )
}
