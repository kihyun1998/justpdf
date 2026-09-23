# CLAUDE.md

## Agent skills

### Issue tracker

Issues are tracked as GitHub issues on `kihyun1998/justpdf` via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Uses the canonical five-role label vocabulary unchanged (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context layout — one `CONTEXT.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.

### Map

The repo's dependency map lives in `docs/map/` — start at `docs/map/README.md`.

### Comments

A comment says what the code is. Why it is this way, what it deliberately leaves out, the trap and the measured value go to the territory note under `docs/map/territory/`; history goes to the commit message. Comments written before this rule still carry the rest: never delete one whose content the map does not yet hold — move it first (`decant`).
