# EVE Production Agent v1

EVE Production Agent v1 is a governed local software engineering agent.

Loop:

```text
task -> inspect -> plan -> propose -> approve -> apply -> validate -> report -> pr-summary -> tui
```

OpenAI is represented by an adapter boundary, while rule-based mode remains the mandatory fallback. EVE owns orchestration, state, approval, safe apply, validation, and evidence.

EVE Production Agent v1 prioritizes real software engineering task execution over internal genome/lineage modeling.

Future evolutionary analysis must be based on real task outcomes: tasks, plans, proposals, approvals, apply results, validation results, reports, and PR outcomes.
