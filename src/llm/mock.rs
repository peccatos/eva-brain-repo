use crate::contracts::{LlmRequest, LlmResponse, LlmStatus};
use crate::llm::provider::LlmProvider;

#[derive(Debug, Clone)]
pub struct MockLlmProvider {
    pub response: LlmResponse,
}

impl LlmProvider for MockLlmProvider {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn complete(&self, request: &LlmRequest) -> Result<LlmResponse, String> {
        let mut response = self.response.clone();
        response.request_id = request.request_id.clone();
        if response.provider.is_empty() {
            response.provider = "mock".to_string();
        }
        if response.model.is_empty() {
            response.model = "mock".to_string();
        }
        if response.output_text.is_empty() && response.parsed_json.is_some() {
            response.status = LlmStatus::Completed;
        }
        Ok(response)
    }
}
