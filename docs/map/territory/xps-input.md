# XPS input

## What it is
Reads glyph text from the FixedDocumentSequence of an XPS/OpenXPS package and turns it into a PDF.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — the `xps` feature builds without render.

## Design model
- `render_page` returns a blank white page ("v1: return white page… Full rendering… would go here").
- Text that overflows the page is dropped (`break`).
- Only text is carried over — not layout, images or vectors.

## Code
- `justpdf-formats/src/xps/mod.rs` — `XpsDocument`, `to_pdf`, `render_page`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Content text encoding](../invariant/content-text-encoding.md)

## Blast radius
- [Format document](format-document.md), [Document builder](document-builder.md).

## Known holes / open
- The preview is a blank page.
