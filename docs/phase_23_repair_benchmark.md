# Phase 23 - Repair Benchmark

`repair-bench` measures how well `fix` performs on a small, deterministic suite of local repositories.

## Command

```bash
cargo run -- repair-bench
cargo run -- repair-bench --suite phase21
cargo run -- repair-bench --output .eva/repair-bench
cargo run -- repair-bench --json
```

## Suite

The built-in `phase21` suite contains five cases:

| Case | Expected problem | Result focus |
| --- | --- | --- |
| `missing_ci` | `missing_ci` | adds `.github/workflows/rust-ci.yml` |
| `missing_smoke_test` | `missing_smoke_test` | adds `tests/eve_smoke.rs` |
| `readme_missing_validation` | `missing_readme_validation` | updates `README.md` |
| `simple_missing_module` | `cargo_check_failure` | adds `src/missing_module.rs` |
| `unknown_empty_project` | none | honest no-action handling |

## Metrics

The report includes:

- `total_cases`
- `passed_cases`
- `failed_cases`
- `partial_cases`
- `detection_success_rate`
- `repair_success_rate`
- `validation_success_rate`
- `evidence_success_rate`

## Output layout

Output is written under:

```text
.eva/repair-bench/<bench_id>/
```

Typical files:

- `request.json`
- `report.json`
- `report.md`
- `cases/<case_id>/result.json`

## Safety rules

- no network access is required
- no OpenAI calls are made
- no git push or merge is performed
- no PR creation is performed
- no real user repositories are mutated
- no new repair behavior is added

## Non-goals

- no campaign mode
- no evolution mode
- no doctor changes
- no fix refactor
- no automatic `.gitignore` mutation beyond output hygiene
