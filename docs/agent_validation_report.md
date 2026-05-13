# Agent Validation And Report

Validation uses an allowlist only:

```text
cargo fmt --check
cargo check
cargo test
```

It does not use shell command strings.

Agent reports are written under `memory/reports/agent-<task_id>.json` and `memory/reports/agent-<task_id>.md`.

Reports include task, workspace, plan, proposal, approval, apply result, validation, changed files, risks, blockers, and next actions.
