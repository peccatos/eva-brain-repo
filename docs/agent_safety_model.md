# Agent Safety Model

Hard rules:

```text
auto_promote=false
operator approval required
no autonomous self-mutation
no git push
no git merge
no system config mutation
no Codex source vendoring
no patch targets under memory/, target/, .git/, releases/, or sandboxes/
```

Codex and ChatGPT inheritance is behavioral only: workflow patterns, contracts, structured output discipline, approval gates, and operator experience.

Codex source trees must stay outside EVE. Optional specimen metadata may reference them, but source copying is forbidden.
