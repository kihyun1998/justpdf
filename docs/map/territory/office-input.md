# Office input (DOCX/XLSX/PPTX)

## What it is
Reads text from OOXML documents and turns it into a PDF.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md)

## Design model
- **The `office` feature does not compile on its own**: for the same reason as [EPUB input](epub-input.md) (`render_page` calls `crate::plaintext`).
- Drops text that overflows ("Would need pagination here for real use").

## Code
- `justpdf-formats/src/office/mod.rs` — `OfficeDocument`, `OfficeType`, `to_pdf`, `render_page`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Content text encoding](../invariant/content-text-encoding.md)

## Blast radius
- [Plaintext input](plaintext-input.md) — a hidden dependency.
- [Crate layering](crate-layering.md), [Format document](format-document.md).

## Known holes / open
- No page breaks; fails to build on its own.
- Tracked: #35 (epub and office standalone builds)
