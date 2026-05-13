# Agent Proposal And Apply

Patch proposals are structured JSON and can be validated without applying.

Allowed patch operations:

```text
CreateFile
AppendFile
ReplaceFileIfExists
ReplaceExactText
```

No proposal can apply itself. The allowed sequence is:

```text
proposal -> operator approval -> safe apply -> validation
```

Safe apply rejects absolute paths, path traversal, `.git/`, `target/`, `memory/`, `releases/`, and `sandboxes/`.

Snapshot and rollback metadata are created before writing files.
