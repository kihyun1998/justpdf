# Crate publishing

## What it is
Publishing to crates.io (each crate), npm (compress-wasm, node, wasm) and PyPI (python), plus the version requirements between crates and the CHANGELOG. The repository's workflows have no publishing step; it is done by hand, and the procedure is not written down.

## Governing decisions
- [ADR-0006](../../adr/0006-breaking-changes-are-allowed-before-1-0.md) — before 1.0 a public-API break is allowed when it gets the design right; it is recorded under **Changed** in `CHANGELOG.md` and the release bumps the minor version (0.x semver).
- [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md) only says the bindings each have their own registry and pipeline; it sets no version policy.

## Design model
Status: ADR-0006 sets the bump rule; the rest is read from the manifests and the registries.
- Published state (crates.io API, measured 2026-10-30): `justpdf-core` 0.1.4; `justpdf`, `justpdf-render`, `justpdf-formats`, `justpdf-special`, `justpdf-cli`, `justpdf-ffi` 0.1.2. `justpdf-compress-wasm`, `justpdf-wasm`, `justpdf-node`, `justpdf-python` are not on crates.io; `justpdf`, `justpdf-wasm` and `justpdf-compress-wasm` are not on npm; the PyPI name `justpdf` belongs to an unrelated project (Pujan-Dev/justpdf, 0.0.2). No `v*` tag exists, so no binary release has been made ([Release](release.md)).
- Each crate has its own version. Downstream crates declare a caret requirement on core and render together with the path (`version = "…", path = …`). When core bumps its patch version the requirement string stays at the old value — at HEAD core is 0.1.4 and every dependent still says `0.1.3`, still satisfied by the caret. Each `Cargo.toml` owns its current value: `git grep -n 'justpdf-[a-z]* = {' -- '*/Cargo.toml'`. Under ADR-0006 a breaking core release bumps the minor version (0.2), which a `0.1.x` caret does not accept, so every dependent's requirement must be raised in the same change (inferred).
- `justpdf-compress-wasm` depends on core by `path` only, with no `version` and no `publish = false`; `cargo publish --dry-run -p justpdf-compress-wasm` fails: "all dependencies must have a version requirement specified when publishing" (measured 2026-10-30).
- `justpdf-core` has no `include`/`exclude`, so its `tests/fixtures` (a 431 KB font among them) ship in the crate (#66, measured 2026-09-23). No workspace manifest has `include`/`exclude` (read 2026-10-30).
- Binding package metadata is kept apart from the crate version: `justpdf-node/package.json` says `0.1.0` against `Cargo.toml` `0.1.2`; `justpdf-python/pyproject.toml` has no version at all, which stops maturin (#201).
- The CHANGELOG's `[Unreleased]` collects what has merged since core 0.1.4. Which crate a heading belongs to lives only in the parenthetical on the heading (`## [0.1.4] - 2026-05-08 (justpdf-core)`); the 0.1.2 releases of the other crates have no heading of their own (read 2026-10-30).

## Code
- `CHANGELOG.md` — `[Unreleased]`, `[0.1.4]`, `[0.1.3]`, `[0.1.0]`
- `justpdf-compress-wasm/Cargo.toml` — `justpdf-core` (path only)
- `justpdf-core/Cargo.toml` — `version`
- `justpdf-node/package.json` — `version`
- `justpdf-python/pyproject.toml` — `[project]`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Compress WASM](compress-wasm.md) — its path-only core dependency makes it unpublishable to crates.io; it is meant for npm.
- [Language bindings](language-bindings.md) — each binding has its own registry and its own version field, none tied to the crate version.
- [Python binding](python-binding.md) — the PyPI name it would publish under is taken.
- [Release](release.md) — the binary's version is the tag, set by hand apart from the crates.
- [Published docs](published-docs.md) — a crate's README becomes its crates.io page, and the install lines in the READMEs name registry packages; a rename or a missing package makes them wrong.
- [Facade](facade.md) — ADR-0006's minor bump on a break applies to the facade's re-exports of core and render too.

## Known holes / open
- The publishing procedure is not written down anywhere, and no workflow publishes.
- `justpdf-compress-wasm` cannot be published to crates.io as it stands (above); whether it should be (or should carry `publish = false`) is an open question for the maintainer.
- The PyPI name `justpdf` is taken; the npm names the READMEs use are unpublished (above). Open question for the maintainer: under which names the bindings will be published.
- Test fixtures ship in the core crate. Tracked: #66
