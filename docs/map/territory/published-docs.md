# Published docs (README, crate READMEs, mdBook)

## What it is
The documentation surfaces people outside the repository read: the root README (GitHub, release archives), each crate's README (its crates.io page), and the `docs/` mdBook guide. Nothing checks these documents when the code changes.

## Governing decisions
**None.**

## Design model
- On 2026-09-23 the examples and feature lists in the README, crate READMEs and mdBook were brought in line with the code. Rust examples were checked by compiling them with `cargo check` in a temporary crate outside the repository, and CLI examples were compared against the built binary's `--help`. Python, Node, WASM and C examples were compared by reading the binding sources and headers (not run).
- **That check happened once.** There is no mechanism that compiles or runs the doc examples (doctest, mdBook build, CI job), so when the public API changes the docs quietly drift again.
- Known unimplemented parts are stated in the docs as they are (e.g. CLI `sign` does nothing, standard fonts are ASCII only).
- The mdBook is not built or published anywhere, neither in CI nor from the README.
- Other prose documents in the repository (`dev/pdf-compress-wasm-design.md`, `docs/mupdf-feature-analysis.md`, `CHANGELOG.md`) had their false sentences fixed the same day. The old `roadmap.md` did not match the code and was deleted.

## Code
- `README.md` — `render_png`
- `docs/book.toml` — `book`
- `docs/src/SUMMARY.md` — table of contents

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- Every note that changes the public API — [Facade](facade.md), [CLI](cli.md), [Language bindings](language-bindings.md), [Format document](format-document.md), [Document modifier](document-modifier.md). Edges from this map to the documentation surface converge here.
- [CI](ci.md) — there is no doctest or mdBook build.
- [Release](release.md) — the root README goes into the archive.

## Known holes / open
- There is no mechanism that compiles the examples. The cheapest gate is to move the README's Rust examples into `justpdf/examples/` or make them doctests.
- Tracked: #59 (CI gate gaps)
