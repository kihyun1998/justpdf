# Text wrapping

## What it is
A writing-side utility that wraps and aligns text to a given width and measures width. It lives under `text/`, but it belongs to generation, not extraction.

## Governing decisions
**None.**

## Design model
- `char_width` uses the Unicode scalar as the character code — widths are wrong for fonts with a non-Identity encoding (inferred).

## Code
- `justpdf-core/src/text/text_layout.rs` — `layout_text`, `measure_text_width`, `char_width`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Document builder](document-builder.md) — the side that would use wrapping (not connected today).
- [Format document](format-document.md) — each input format does its own line breaking.

## Known holes / open
- No caller outside `text/`.
