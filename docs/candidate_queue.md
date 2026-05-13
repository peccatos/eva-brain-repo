# EVA Candidate Queue

Candidates are never deleted by queue hygiene. They are classified with explicit state and reason:

- `Ready`
- `Blocked`
- `Quarantined`
- `Stale`
- `Legacy`
- `Duplicate`
- `Unreplayable`
- `AlreadyPromoted`
- `Unknown`

Examples:

- failed replay becomes `Unreplayable`
- duplicate candidate becomes `Duplicate`
- already promoted candidate becomes `AlreadyPromoted`
- missing report/evidence becomes `Stale` or `Blocked`
- cosmetic or unsafe mutation becomes `Quarantined`

The queue summary is visible through promotion queue output and the TUI candidate screen.
