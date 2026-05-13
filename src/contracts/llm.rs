use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LlmPurpose {
    Plan,
    ProposePatch,
    ReviewPatch,
    ExplainValidation,
    GenerateReport,
    GeneratePrSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LlmStatus {
    Completed,
    Failed,
    Refused,
    NotConfigured,
    FallbackUsed,
    MalformedOutput,
    BlockedBySanitizer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    pub request_id: String,
    pub purpose: LlmPurpose,
    pub system_prompt: String,
    pub input: String,
    pub expected_schema: String,
    pub max_output_tokens: u32,
    pub temperature: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub request_id: String,
    pub provider: String,
    pub model: String,
    pub status: LlmStatus,
    pub output_text: String,
    pub parsed_json: Option<serde_json::Value>,
    pub warnings: Vec<String>,
    pub blockers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SanitizedContext {
    pub text: String,
    pub redactions: Vec<String>,
    pub blocked: bool,
    pub blockers: Vec<String>,
}
