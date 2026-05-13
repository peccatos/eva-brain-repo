# OpenAI LLM Adapter

The OpenAI adapter is the primary provider boundary for plans, patch proposals, explanations, reports, and PR summaries.

Environment:

```text
OPENAI_API_KEY
EVE_LLM_PROVIDER=openai
EVE_OPENAI_MODEL=gpt-5.5
EVE_LLM_MODE=openai|rule_based
```

If `OPENAI_API_KEY` is missing, EVE reports `provider=rule_based` and continues through deterministic fallback.

The adapter must not print or store API keys. It must sanitize context before any provider call and block `.env`, secret-like input, `memory/`, `.git/`, `target/`, `releases/`, and `sandboxes/`.
