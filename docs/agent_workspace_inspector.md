# Agent Workspace Inspector

The inspector reads local metadata only.

It detects:

```text
Cargo.toml
Cargo.lock
src/main.rs
src/lib.rs
tests/
docs/
README.md
git status
branch
HEAD
```

If git metadata is unavailable, inspection records `unknown` instead of failing.
