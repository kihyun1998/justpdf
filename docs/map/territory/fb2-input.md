# FB2 input

## What it is
Turns the section text of FictionBook 2 XML into a PDF.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — `fb2` pulls in render for its preview (#2).

## Design model
- Drops text that overflows.

## Code
- `justpdf-formats/src/fb2/mod.rs` — `Fb2Document`, `to_pdf`, `render_page`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Content text encoding](../invariant/content-text-encoding.md)

## Blast radius
- [CLI](cli.md) — `convert` reads `.fb2` (measured, `fb2_converts_to_pdf`).
- [Render API](render-api.md).

## Known holes / open
**None.**
