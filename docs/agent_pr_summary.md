# Agent PR Summary

PR summary generation is local metadata only.

It writes:

```text
memory/pr_summaries/<task_id>.json
memory/pr_summaries/<task_id>.md
```

It does not run `git push`, `git merge`, `gh pr create`, or any network PR operation.

Sections:

```text
Summary
Changes
Validation
Safety
Risks
Notes
```
