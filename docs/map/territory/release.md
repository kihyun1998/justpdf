# Release

## What it is
Pushing a `v*` tag is the only trigger. It builds only `justpdf-cli` in release mode on four targets (Linux x86_64, macOS x86_64 and aarch64, Windows x86_64), bundles it with the README and licenses, and attaches it to the GitHub Release of the same name.

## Governing decisions
- [ADR-0003](../../adr/0003-cli-is-end-product-not-a-dependency-layer.md) — the CLI is the single end product, so it is the only binary distributed.

## Design model
- Publishing to crates.io, PyPI and npm is not in this workflow ([Crate publishing](crate-publishing.md)).
- It does not check that the tag matches the crate versions.

## Code
- `.github/workflows/release.yml` — `build`, `softprops/action-gh-release`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [CLI](cli.md) — what gets distributed.
- [Crate publishing](crate-publishing.md) — where the version number comes from.
- [Published docs](published-docs.md) — the archive includes the root README (the README whose examples are wrong).

## Known holes / open
**None.** Writing this note turned up no holes.
