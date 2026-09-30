# MOBI input

## What it is
Reads text from Mobipocket files (PalmDOC-compressed or uncompressed).

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — the `mobi` feature pulls in render for its preview (#2).

## Design model
- Any other compression is an error.

## Code
- `justpdf-formats/src/mobi/mod.rs` — `MobiDocument`, `to_pdf`, `render_page`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Content text encoding](../invariant/content-text-encoding.md)

## Blast radius
- [CLI](cli.md) — `convert` has no MOBI branch.
- [Render API](render-api.md) — preview.

## Known holes / open
- Unreachable from the CLI (above).
- Tracked: #49 (convert MOBI and FB2)
