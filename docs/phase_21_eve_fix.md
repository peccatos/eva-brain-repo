# Phase 21 - EVE Fix

`fix` is a safe facade over the governed repair pipeline.

## Commands

```bash
cargo run -- fix .
cargo run -- fix . --apply
cargo run -- fix . --only ci
cargo run -- fix . --only tests
cargo run -- fix . --no-llm
```

## Semantics

- Default mode is dry-run.
- Exactly one actionable issue is selected per run.
- Phase 21 priority is:
  1. cargo-check failure
  2. missing CI
  3. missing smoke test
  4. missing README validation section
- Files are mutated only with `--apply`.

## Safety model

- No git push.
- No git merge.
- No PR creation.
- No system configuration changes.
- No daemon or background loop.
- No high-risk fixes.
- No deletion of user files.
- OpenAI is optional; deterministic rule-based fallback must work.

## Evidence layout

Evidence is written under:

```text
.eva/fix/<fix_id>/
```

Typical files:

```text
request.json
detection.json
proposal.json
dry_run.json
apply_result.json
validation.json
report.md
report.json
```

## Non-goals

- autonomous campaigns
- self-evolution loops
- system mutation
- GitHub automation
- large repo rewrites
