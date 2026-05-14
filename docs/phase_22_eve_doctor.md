# Phase 22 - EVE Doctor

`doctor` is a read-only diagnostic facade over the target project.

## Commands

```bash
cargo run -- doctor .
cargo run -- doctor . --json
cargo run -- doctor . --validate
cargo run -- doctor . --no-llm
```

## Semantics

- Default mode is read-only.
- No source mutation is performed by default.
- `--validate` is optional and may run `cargo fmt --check`, `cargo check --all-targets`, and `cargo test` for Rust projects only.
- Validation side effects such as `Cargo.lock` are recorded separately.
- Invalid targets are blocked before evidence creation.

## Findings

Doctor reports deterministic findings such as:

- Cargo.toml present or missing
- Rust CI workflow present or missing
- smoke test present or missing
- README validation section present or missing
- workspace clean or dirty

## Suggestions

Doctor suggests safe follow-up fixes such as:

- `cargo run -- fix <target> --only ci`
- `cargo run -- fix <target> --only tests`
- `cargo run -- fix <target> --only docs`

`--apply` is not suggested by default.

## Evidence layout

Evidence is written under:

```text
<target>/.eva/doctor/<doctor_id>/
```

Typical files:

- `request.json`
- `report.json`
- `report.md`
- `validation.json` when `--validate` is used

## Health score

The report uses a simple 0-100 health score:

- critical finding: -40
- warn finding: -10
- info finding: -2
- ok finding: -0

Status is derived from the score and the presence of critical findings.

## Non-goals

- no repair application
- no PR creation
- no git push or merge
- no daemon or campaign mode
- no self-evolution
- no automatic source edits
