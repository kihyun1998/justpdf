# Font loading

## What it is
Turns a font dictionary into a `FontInfo` (BaseFont, Subtype, Encoding, widths, descriptor). Uses the standard 14 fonts' width tables as defaults. Text extraction and the renderer both start from this function, but each adds its own supplements on top.

## Governing decisions
**None.**

## Design model
- ToUnicode is not resolved here ("Resolved later by the document").
- Default width: 600 for the standard 14, otherwise 1000. The standard 14 width tables are approximations ("Simplified Helvetica widths"; Symbol/ZapfDingbats are all 500).
- **CID widths (`/W`) are not filled in here**. Only `parse_cid_widths` on the text extraction side fills `FontWidths::CID`. The renderer calls `parse_font_info` on the descendant font and so gets `FontWidths::None` — the same Type0 font advances differently in rendering and in text ([Font resolution](../invariant/font-resolution.md)).

## Code
- `justpdf-core/src/font/mod.rs` — `FontInfo`, `FontDescriptor`, `FontWidths`, `CIDWidthEntry`, `get_width`, `parse_font_info`, `parse_font_descriptor`, `parse_widths`
- `justpdf-core/src/font/standard14.rs` — `is_standard14`, `standard14_widths`, `strip_subset_prefix`

## Reference behaviour
**None.** The code cites "PDF spec 7.6" and "Table 123". Clauses to compare against: ISO 32000-2 §9.6 (simple fonts), §9.7 (composite fonts), §9.8 (descriptors).

## Cross-cutting invariants
- [Font resolution](../invariant/font-resolution.md)

## Blast radius
- [Text extraction](text-extraction.md), [Glyph rendering](glyph-rendering.md), [SVG renderer](svg-renderer.md) — three consumers, each adding its own supplements on top of this result.
- [Font encodings](font-encodings.md) — `parse_encoding` lives in the same file.
- [CID fonts](cid-fonts.md) — the actual source of CID widths.
- [Font subsetting](font-subsetting.md) — shares the `/Widths` indexing rule.
- [Plaintext input](plaintext-input.md) — hardcodes the Courier width 0.6 instead of calling this table.

## Known holes / open
- The comment "CID font: /W array (not yet fully parsed)" is still true of this function.
