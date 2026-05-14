# Phase 24X - Repair Quality, Capability, and External Proof

Phase 24X combines quality control, safe capability expansion, and local external proof in one controlled step.

## 24X.1 Repair Bench History + Regression Gate

The repair bench now records persistent local history under `.eva/repair-bench/` and can compare a fresh run against a baseline before new repair behavior is trusted.

Commands:

```bash
cargo run -- repair-bench
cargo run -- repair-bench-history
cargo run -- repair-bench-gate
```

Regression rules:

- fail if `failed_cases` increases
- fail if `passed_cases` decreases
- fail if a previously passing actionable case becomes failed
- fail if actionable `detection_success_rate` decreases
- fail if actionable `repair_success_rate` decreases
- fail if actionable `validation_success_rate` decreases
- fail if actionable `evidence_success_rate` decreases
- do not fail only because `unknown_empty_project` remains partial

Metrics:

- `actionable_cases` excludes the honest no-op `unknown_empty_project`
- `repair_success_rate` and `validation_success_rate` use actionable cases as their denominator
- `evidence_success_rate` still tracks all cases

## 24X.2 Repair Capability Expansion

Three deterministic, no-LLM repair capabilities were added:

- `missing_gitignore_target`
- `missing_clippy_ci`
- `missing_readme_usage_section`

Safety rules:

- source mutation only happens with `--apply`
- dry-run mode stays read-only
- only `.gitignore`, `.github/workflows/rust-ci.yml`, and `README.md` are touched for these cases
- no broad safe-path expansion was added

## 24X.3 External Trial Pack

The external trial pack runs doctor plus fix dry-runs against local repositories only.

Command:

```bash
cargo run -- external-trial /path/to/repo
```

Rules:

- no clone or fetch
- no network requirement
- no OpenAI requirement
- dry-run by default
- no source mutation by default
- no push, merge, PR, or campaign behavior

Non-goals:

- no autonomous evolution expansion
- no auto-promote
- no daemon or systemd changes
- no TUI redesign
- no OpenAI-only repair path
