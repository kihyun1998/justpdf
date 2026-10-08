# Font loading

## What it is
Turns a font dictionary into a `FontInfo` (BaseFont, Subtype, Encoding, widths, descriptor). Uses the standard 14 fonts' width tables as defaults. Text extraction and the renderer both start from this function, but each adds its own supplements on top.

## Governing decisions
**None.**

## Design model
- `parse_font_info` takes a bare `PdfDict` and reads a reference as absent. Indirect entries are resolved one step before it, in core, by `resolve_font_entries(dict, resolve)`: `/Widths` (and each indirect element of the array — MuPDF's `pdf_array_get_*` resolve elements too, read raw 2026-10-08), `/FirstChar`, `/LastChar` and `/Encoding` are replaced by what `resolve` (`FnMut(&IndirectRef) -> Option<PdfObject>`) returns. Text extraction (`resolve_fonts`), the raster renderer and the SVG renderer pass `PdfDocument::resolve`; font subsetting (`simple_font_glyph_ids`) passes `DocumentModifier::find_object_pub`. It returns the resolved dictionary rather than a `FontInfo`, because every consumer also reads the dictionary (`/Encoding` as a name or a dictionary, `/ToUnicode`, `/FontDescriptor`). A reference `resolve` cannot answer is left in place, so `parse_font_info` reads it as absent (the result before #287) and subsetting still sees a reference and keeps the font whole (`Some(_) => return None`); dropping it instead would let subsetting choose glyphs through StandardEncoding. Until #287 each consumer inlined an indirect `/Encoding` on its own, the SVG renderer none, and an indirect `/Widths` fell back to the default width everywhere (measured 2026-10-08: arXiv 1706.03762, pdfTeX, all 13 Type1 fonts carry `/Widths N 0 R`; rendered glyphs spread apart and overlap; `tests/font_widths.rs`: `[10, 16.67, 23.34]` for `[10, 11, 31]`).
- ToUnicode is not resolved here ("Resolved later by the document").
- Default width: 600 for the standard 14, otherwise 1000. The standard 14 width tables are approximations ("Simplified Helvetica widths"; Symbol/ZapfDingbats are all 500).
- On a `CIDFontType0`/`CIDFontType2` dictionary, `parse_font_info` fills `FontWidths::CID` from `/W` and `/DW` ([CID fonts](cid-fonts.md)); a Type0 dictionary itself gets `FontWidths::None`, and its consumers take the descendant's widths.

## Code
- `justpdf-core/src/font/mod.rs` — `FontInfo`, `FontDescriptor`, `FontWidths`, `CIDWidthEntry`, `get_width`, `parse_font_info`, `resolve_font_entries`, `parse_font_descriptor`, `parse_widths`
- `justpdf-core/src/font/standard14.rs` — `is_standard14`, `standard14_widths`, `strip_subset_prefix`
- `justpdf-core/tests/font_widths.rs` — `indirect_widths_place_the_glyphs`, `an_indirect_first_char_is_read`, `indirect_width_elements_are_read`, `a_widths_reference_that_does_not_resolve_is_ignored`
- `justpdf-render/tests/render_font_widths.rs` — `raster_glyphs_advance_by_indirect_widths`, `svg_glyphs_advance_by_indirect_widths`

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
- A font's `/FontDescriptor` is not passed through `resolve_font_entries` (`FontInfo::descriptor` has no consumer).
