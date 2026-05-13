# EVA Metrics Semantics

EVA separates real failures from successful safety behavior.

`failed_runs` counts only actual runtime failures:

- real execution failure
- cargo gate failure
- replay failure

Safety rejections are tracked separately:

- duplicate safety rejection
- cosmetic rejection
- policy rejection

`duplicate_rejected=true` increments `duplicate_rejected_runs` and `safety_rejected_runs`, but it does not increment `failed_runs`. A duplicate rejection means the guard worked.

The main ratios are:

- `pass_ratio = passed_runs / total_runs`
- `effective_failure_ratio = failed_runs / total_runs`
- `safety_rejection_ratio = safety_rejected_runs / total_runs`
