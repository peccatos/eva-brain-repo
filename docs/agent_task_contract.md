# Agent Task Contract

An `AgentTask` records a real operator task with approval required by default.

Default scope:

```text
src/
tests/
docs/
README.md
```

Forbidden patch targets:

```text
.git/
target/
memory/
releases/
sandboxes/
.eva-runtime-tests/
.eva-evolution-tests/
```

Create a task:

```bash
cargo run -- task "document production agent v1"
```
