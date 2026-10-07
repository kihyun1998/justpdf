# Office input (DOCX/XLSX/PPTX)

## What it is
Reads text from OOXML documents and turns it into a PDF.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md)

## Design model
- As in [EPUB input](epub-input.md), the preview borrows `crate::plaintext` only under `cfg(feature = "plaintext")` and otherwise returns `FormatError::Format` after the index check; `office` compiles on its own (measured 2026-10-07). Maintainer's call in #35.
- Drops text that overflows ("Would need pagination here for real use").

## Code
- `justpdf-formats/src/office/mod.rs` — `OfficeDocument`, `OfficeType`, `to_pdf`, `render_page`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Content text encoding](../invariant/content-text-encoding.md)

## Blast radius
- [Plaintext input](plaintext-input.md) — whose preview it borrows when the `plaintext` feature is on.
- [Crate layering](crate-layering.md), [Format document](format-document.md).

## Known holes / open
- No page breaks.
