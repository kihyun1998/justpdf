# CJK font embedding (writing)

## What it is
The writing-side code that wraps a TrueType font as a CID font (Type0 + CIDFontType2) and embeds it in a document. It includes language ordering detection, `/W` array generation and ToUnicode CMap generation.

## Governing decisions
**None.**

## Design model
- Writes `/CIDToGIDMap /Identity`.
- Writes a default width of 1000 for every glyph ("For now, use a default width of 1000 for all glyphs").
- Splits bfchar sections into chunks of 100 (code comment: "PDF spec limit").

## Code
- `justpdf-core/src/font/cjk.rs` — `build_cid_font`, `detect_ordering`, `CJKOrdering`, `generate_to_unicode_cmap`, `build_w_array`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Content text encoding](../invariant/content-text-encoding.md) — the only material for writing non-ASCII text correctly is here, but it is not connected.

## Blast radius
- [CID fonts](cid-fonts.md), [ToUnicode](tounicode.md) — the side that reads the fonts made here.
- [Document builder](document-builder.md) — the side this should be connected to (`show_text` assumes standard fonts only).

## Known holes / open
- Nothing outside `cjk.rs` calls it. Every path that writes non-ASCII text, formats and OCR among them, uses a standard Type1 font instead of this module.
- Tracked: #34 (non-ASCII content text)
