use crate::contracts::{LlmRequest, LlmResponse, LlmStatus};
use crate::llm::provider::LlmProvider;

#[derive(Debug, Clone, Default)]
pub struct RuleBasedLlmProvider;

impl LlmProvider for RuleBasedLlmProvider {
    fn name(&self) -> &'static str {
        "rule_based"
    }

    fn complete(&self, request: &LlmRequest) -> Result<LlmResponse, String> {
        Ok(LlmResponse {
            request_id: request.request_id.clone(),
            provider: self.name().to_string(),
            model: "rule_based".to_string(),
            status: LlmStatus::FallbackUsed,
            output_text: "rule_based fallback generated deterministic agent output".to_string(),
            parsed_json: None,
            warnings: vec!["openai_not_used".to_string()],
            blockers: Vec::new(),
        })
    }
}
