# Before 1.0, a change that gets the design right may break the public API

Until 1.0, a change that brings the code closer to the right design may break the public API of any crate. The break is recorded under **Changed** in `CHANGELOG.md`, and the release bumps the minor version (0.x semver). Avoiding a break is not a reason to keep a worse design, to add a compatibility shim, or to narrow what a crate exposes.

## Why

Nobody outside this project depends on these crates yet (measured 2026-10-24 on crates.io: `justpdf-render` has 649 downloads, and its only reverse dependencies are `justpdf`, `justpdf-cli`, `justpdf-ffi`, `justpdf-formats` and `justpdf-special`; `justpdf` has none). A break costs almost nothing now, and a design kept to avoid one is paid for in every later change.

Internal modules already change under this rule. `justpdf-render` exposes most of its modules (`device`, `graphics_state`, `interpreter`, `glyph`, `glyph_cache`, …) as `pub mod`, and the facade re-exports the crate as `justpdf::render`. Breaks to them are recorded under Changed (#127, #163, #223, #50).

## Trade-offs

The alternative considered was to make `justpdf-render`'s internal modules `pub(crate)`, so that internal refactors stop being API breaks (#371). It was rejected: its only gain is avoiding breaks, which this ADR accepts. What would remain (a smaller docs.rs page, and the compiler flagging unused internals as dead code) does not justify the work now.

## Consequences

- When a change needs a break, the change does not shrink to avoid it. The question it answers is "is this the right design", not "does this break someone".
- Every break still gets its Changed entry. That is the only cost a break carries before 1.0.
- This ADR is revisited before 1.0. At that point the public surface becomes a promise, and narrowing it (as #371 proposed) is the work to reconsider.
