# Plaintext input

## What it is
Splits plain text into pages and builds a PDF in Courier. EPUB and Office borrow its preview render implementation.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — `plaintext` pulls in render (#2).

## Design model
- Hardcodes the glyph width as `font_size * 0.6` (matches Courier's 600, but does not call the core table — [Font loading](font-loading.md)).
- `render_page` builds a one-page PDF and draws it with render. EPUB and Office call this module, so **this module's feature gate decides whether three formats build**.

## Code
- `justpdf-formats/src/plaintext/mod.rs` — `PlainTextDocument`, `to_pdf`, `render_page`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Content text encoding](../invariant/content-text-encoding.md)

## Blast radius
- [EPUB input](epub-input.md), [Office input](office-input.md) — hidden callers.
- [Crate layering](crate-layering.md) — the feature gate.
- [Render API](render-api.md).

## Known holes / open
- Tracked: #35 (epub and office standalone builds)
