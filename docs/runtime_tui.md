# EVA Runtime TUI

Run the read-only operator console:

```bash
cargo run -- tui
```

The flag form is also supported:

```bash
cargo run -- --tui
```

In an interactive terminal the TUI accepts:

```text
q / Esc = quit
1 = Dashboard
2 = Runs
3 = Candidates
4 = Metrics
5 = Release
6 = Logs
7 = Help
r = refresh
h = help
```

In non-interactive mode it prints one deterministic dashboard snapshot and exits. Opening the TUI does not mutate source files, candidates, releases, sandboxes, or external repositories.
