use crate::contracts::{LlmRequest, LlmResponse};

pub trait LlmProvider {
    fn name(&self) -> &'static str;
    fn complete(&self, request: &LlmRequest) -> Result<LlmResponse, String>;
}
