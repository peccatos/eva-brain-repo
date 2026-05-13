# Phase 16 Evolution Core Readiness

Phase 16 is not active yet.

This repository now exposes a readiness scaffold only:

- `runtime_green`
- `approved_release_candidate`
- `release_bundle_exists`
- `tui_hydration_ok`
- `metrics_truth_ok`
- `candidate_queue_truth_ok`
- `phase_16_allowed`
- `blockers`

Rule:

```text
phase_16_allowed = true only if runtime_validation.status == green
```

That means:

- no sandbox leaks
- no critical blockers
- metrics semantics are clean
- candidate queue truth is visible
- a real approved release candidate exists
- a real release bundle exists

This phase does not start mutation graphs, lineage inheritance, recombination engines, or autonomous evolution campaigns.
