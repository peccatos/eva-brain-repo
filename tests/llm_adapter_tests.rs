#[path = "evolution_test_support.rs"]
mod evolution_test_support;

use std::sync::{Mutex, OnceLock};

use eva_runtime_with_task_validator::contracts::{LlmPurpose, LlmRequest, LlmStatus};
use eva_runtime_with_task_validator::llm::openai::responses_api_payload;
use eva_runtime_with_task_validator::llm::{
    llm_health, sanitize_llm_context, LlmProvider, OpenAiLlmProvider, RuleBasedLlmProvider,
};

#[test]
fn missing_openai_key_falls_back_to_rule_based_health_without_key_leak() {
    let _guard = env_lock().lock().expect("env lock");
    std::env::remove_var("OPENAI_API_KEY");
    std::env::remove_var("EVE_LLM_MODE");
    std::env::remove_var("EVE_LLM_PROVIDER");
    let health = llm_health();
    assert!(health.contains("provider=rule_based"));
    assert!(health.contains("fallback_available=true"));
    assert!(!health.contains("sk-"));
}

#[test]
fn openai_key_selects_openai_health_without_key_leak() {
    let _guard = env_lock().lock().expect("env lock");
    std::env::set_var("OPENAI_API_KEY", "sk-test-secret");
    std::env::remove_var("EVE_LLM_MODE");
    std::env::remove_var("EVE_LLM_PROVIDER");
    let health = llm_health();
    std::env::remove_var("OPENAI_API_KEY");
    assert!(health.contains("provider=openai"));
    assert!(health.contains("openai_configured=true"));
    assert!(!health.contains("sk-test-secret"));
}

#[test]
fn rule_based_provider_completes_without_network() {
    let provider = RuleBasedLlmProvider;
    let response = provider.complete(&request("hello")).expect("response");
    assert_eq!(response.status, LlmStatus::FallbackUsed);
    assert_eq!(response.provider, "rule_based");
}

#[test]
fn sanitizer_redacts_obvious_secrets_and_blocks_env_like_content() {
    let sanitized =
        sanitize_llm_context("Authorization: Bearer secret\n.env\nOPENAI_API_KEY=sk-test");
    assert!(sanitized.blocked);
    assert!(sanitized
        .redactions
        .iter()
        .any(|item| item.contains("authorization")));
    assert!(sanitized.blockers.iter().any(|item| item.contains(".env")));
    assert!(!sanitized.text.contains("sk-test"));
}

#[test]
fn openai_provider_blocks_forbidden_context_before_network() {
    let provider = OpenAiLlmProvider::default();
    let response = provider
        .complete(&request("memory/tasks/foo.json"))
        .expect("response");
    assert_eq!(response.status, LlmStatus::BlockedBySanitizer);
}

#[test]
fn openai_responses_payload_uses_structured_json_schema() {
    let request = LlmRequest {
        request_id: "req".to_string(),
        purpose: LlmPurpose::ProposePatch,
        system_prompt: "system".to_string(),
        input: "input".to_string(),
        expected_schema: "PatchProposal".to_string(),
        max_output_tokens: 512,
        temperature: 0.0,
    };
    let payload = responses_api_payload(&request, "safe input", "gpt-5.5");
    assert_eq!(payload["model"], "gpt-5.5");
    assert_eq!(payload["text"]["format"]["type"], "json_schema");
    assert_eq!(payload["text"]["format"]["name"], "PatchProposal");
    assert_eq!(payload["text"]["format"]["strict"], true);
}

fn request(input: &str) -> LlmRequest {
    LlmRequest {
        request_id: "llm-test".to_string(),
        purpose: LlmPurpose::Plan,
        system_prompt: "test".to_string(),
        input: input.to_string(),
        expected_schema: "schema".to_string(),
        max_output_tokens: 128,
        temperature: 0.0,
    }
}

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
