use crate::contracts::SanitizedContext;

pub fn sanitize_llm_context(input: &str) -> SanitizedContext {
    let mut text = input.to_string();
    let mut redactions = Vec::new();
    let mut blockers = Vec::new();
    let lower = input.to_ascii_lowercase();
    for marker in [
        "authorization:",
        "bearer ",
        "openai_api_key",
        "ssh private key",
        "password",
        "api_key",
        "token",
        "secret",
    ] {
        if lower.contains(marker) {
            redactions.push(marker.to_string());
            text = redact_case_insensitive(&text, marker);
        }
    }
    for marker in [".env", "*.pem", "*.key", "id_rsa", "id_ed25519"] {
        if lower.contains(marker.trim_start_matches('*')) {
            blockers.push(format!("secret_like_context:{marker}"));
        }
    }
    for marker in [".git/", "target/", "memory/", "releases/", "sandboxes/"] {
        if lower.contains(marker) {
            blockers.push(format!("forbidden_context:{marker}"));
        }
    }
    SanitizedContext {
        text,
        redactions,
        blocked: !blockers.is_empty(),
        blockers,
    }
}

fn redact_case_insensitive(input: &str, needle: &str) -> String {
    input
        .split_whitespace()
        .map(|part| {
            if part.to_ascii_lowercase().contains(needle) {
                "[REDACTED]"
            } else {
                part
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
