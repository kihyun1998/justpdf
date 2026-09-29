# BiDi

## What it is
Uses `unicode-bidi` to compute a string's direction runs and decide whether it contains RTL.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md)

## Design model
- No writing path uses it — RTL text is not reordered when written to PDF.

## Code
- `justpdf-special/src/bidi/mod.rs` — `resolve_bidi`, `contains_rtl`, `Direction`, `BidiRun`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Document builder](document-builder.md), [Text extraction](text-extraction.md) — candidates to connect to (none today).

## Known holes / open
- No consumer inside the repository.
