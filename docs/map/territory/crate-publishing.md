# Crate publishing

## What it is
Publishing to crates.io (each crate), npm (compress-wasm), and PyPI and npm (bindings), plus the version requirements between crates and the CHANGELOG. The repo's workflows have no publishing automation; it is done by hand.

## Governing decisions
**None.** [ADR-0002](../../adr/0002-language-bindings-outside-workspace.md) only says the bindings each have their own registry and pipeline; it sets no version policy.

## Design model
- Each crate has its own version. Downstream crates declare a caret requirement on core (`version = "…", path = …`) — when core bumps its patch version the requirement string stays at the old value (it is still satisfied, being a caret). Each `Cargo.toml` owns its current value: `grep -n 'justpdf-core' */Cargo.toml`.
- `justpdf-compress-wasm` depends on core by `path` only, with no `version` and no `publish = false` — a shape `cargo publish` would reject (inferred, not run).
- The CHANGELOG's `[Unreleased]` collects what has merged since 0.1.4. Which crate version a CHANGELOG heading corresponds to (which entry belongs to which crate's release) lives only in the parenthetical note on the heading.

## Code
- `CHANGELOG.md` — `Unreleased`
- `justpdf-compress-wasm/Cargo.toml` — `justpdf-core`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Compress WASM](compress-wasm.md) — the unpublishable dependency, the npm package.
- [Language bindings](language-bindings.md) — each with its own registry.
- [Release](release.md) — binary versions.
- [Published docs](published-docs.md) — a crate's README becomes its crates.io page.

## Known holes / open
- The publishing procedure is not written down anywhere.
