#[path = "evolution_test_support.rs"]
mod evolution_test_support;

use eva_runtime_with_task_validator::contracts::{LlmPurpose, LlmRequest, LlmStatus};
use eva_runtime_with_task_validator::llm::{
    llm_health, sanitize_llm_context, LlmProvider, OpenAiLlmProvider, RuleBasedLlmProvider,
};

#[test]
fn missing_openai_key_falls_back_to_rule_based_health_without_key_leak() {
    std::env::remove_var("OPENAI_API_KEY");
    let health = llm_health();
    assert!(health.contains("provider=rule_based"));
    assert!(health.contains("fallback_available=true"));
    assert!(!health.contains("sk-"));
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
