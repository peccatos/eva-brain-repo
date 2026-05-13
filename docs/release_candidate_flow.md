# EVA Release Candidate Flow

Release candidate approval is operator-gated.

Approve a candidate only after the candidate is ready:

```bash
cargo run -- --release-approve <RUN_ID>
```

The command checks:

- candidate exists
- candidate state is ready
- replay status is ok
- cargo test/run gates passed
- no promotion blockers are present
- existing governance approval rules still pass

Approval writes metadata under `memory/release_candidates/rc-<RUN_ID>/` and does not promote, push, merge, or mutate source files.

Release bundle generation remains a separate metadata-only step:

```bash
cargo run -- --release-bundle <RUN_ID>
```
