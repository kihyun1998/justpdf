# CLAUDE.md

## Agent skills

### Issue tracker

Issues are tracked as GitHub issues on `kihyun1998/justpdf` via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Uses the canonical five-role label vocabulary unchanged (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context layout — one `GLOSSARY.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.

### Map

The repo's dependency map lives in `docs/map/` — start at `docs/map/README.md`.

- **Read before designing a change:** open the territory note for the code you will touch, follow its `## Blast radius` as a checklist and its `## Cross-cutting invariants`.
- **Write after it lands:** the touched notes' `## Code`, blast edges and holes still match the code.
- **Promotion, at the first fix:** when a fix reveals a fact, ask in grep-able form whether it holds at every site that shares the assumption. If it does, the fix lands with an invariant note under `docs/map/invariant/`, or a new site in an existing one.

### Comments

A comment says what the code is. Why it is this way, what it deliberately leaves out, the trap and the measured value go to the territory note under `docs/map/territory/`. Where none seems to hold it, search the folder and the code symbol first; only then start a new note, named for what the system does there, not for the file. History goes to the commit message. Comments written before this rule still carry the rest: never delete one whose content the map does not yet hold — move it there first.

### Language

Write everything that lands in the repo or the tracker in English: code comments, `docs/map/`, ADRs, issues, PR bodies, commit messages. Talk with the maintainer in the language they write in. In the map, label each fact *measured* (seen by running something) or *inferred* (read from code).
