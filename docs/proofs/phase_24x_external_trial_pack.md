# Phase 24X External Trial Pack

Purpose:

- run read-only diagnostics and fix dry-runs against local repositories
- collect proof artifacts without mutating external source trees by default

Safety rules:

- no network clone or fetch
- no OpenAI key required
- no `git push`, `git merge`, or PR creation
- no apply by default
- no source mutation by default
- no campaign or evolution expansion

Usage:

```bash
cargo run -- external-trial /path/to/cleanrustplayer
cargo run -- external-trial /path/to/tracebox
cargo run -- external-trial /path/to/eve-net-runtime
```

The harness writes report files under `.eva/external-trials/<trial_id>/` and uses dry-run mode by default.

Notes:

- the harness does not clone repositories
- the harness does not fetch repositories
- the harness only works on local paths already present on disk
