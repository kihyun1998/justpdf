# Font loading

## What it is
Turns a font dictionary into a `FontInfo` (BaseFont, Subtype, encoding and `/Differences`, widths, descriptor), using the Standard 14 width tables when a simple font has no `/Widths`. Every reader of a document's fonts starts here: the [Content interpreter](content-interpreter.md) (`load_font`, for text extraction and redaction), the raster and SVG renderers (`resolve_font`), the form appearance encoder (`FontEncoder::new`, [Font encodings](font-encodings.md)) and font subsetting (`simple_font_glyph_ids`). Each adds its own steps on top of the result.

## Governing decisions
**None.** No decision record touches font loading. [ADR 0004](../../adr/0004-redaction-never-under-erases.md) uses the glyph box this area's widths and `/FontBBox` feed, but decides what redaction erases, not how a font is read.

## Design model
Status: from reading the source and the tests; no record.

- **Indirect entries are resolved one step before parsing.** `parse_font_info` takes a bare `PdfDict` and reads a reference as absent. `resolve_font_entries(dict, resolve)` first replaces an indirect `/Widths` (and each indirect element of the array), `/FirstChar`, `/LastChar`, `/Encoding`, `/FontDescriptor`, `/W` (with the arrays and values inside it) and `/DW` by what `resolve` (`FnMut(&IndirectRef) -> Option<PdfObject>`) returns (inferred, read from `resolve_font_entries`). The content interpreter, both renderers and `FontEncoder` pass `PdfDocument::resolve`; subsetting passes `DocumentModifier::find_object_pub`. It returns the resolved dictionary, not a `FontInfo`, because every consumer also reads the dictionary itself (`/Encoding` as a name or a dictionary, `/ToUnicode`, `/FontDescriptor`).
  - A reference `resolve` cannot answer is left in place, so `parse_font_info` reads it as absent (`a_widths_reference_that_does_not_resolve_is_ignored`, measured: passes at 88b2a08, 2026-10-30), and subsetting still sees a reference and keeps the font whole (`simple_font_glyph_ids`: `Some(_) => return None`). Dropping the reference instead would let subsetting choose glyphs through StandardEncoding (inferred).
  - Resolving the elements of `/Widths` follows MuPDF, whose `pdf_array_get_*` resolve elements too (read raw 2026-10-08, #287).
  - Before #287 each consumer inlined an indirect `/Encoding` on its own (the SVG renderer did not), and an indirect `/Widths` fell back to the default width everywhere. Measured 2026-10-08 on arXiv 1706.03762 (pdfTeX: all 13 Type1 fonts carry `/Widths N 0 R`): rendered glyphs spread apart and overlapped, and `tests/font_widths.rs` gave `[10, 16.67, 23.34]` where `[10, 11, 31]` is right (#287).
- `to_unicode` is always `None` out of `parse_font_info` (code comment "Resolved later by the document"); each consumer finds and parses `/ToUnicode` itself ([ToUnicode](tounicode.md)).
- **Widths.** A simple font's `/FirstChar` and `/Widths` give `FontWidths::Simple`, a code below `/FirstChar` or past the array taking the default: 600 for a Standard 14 font, 1000 otherwise. A Standard 14 font with no `/Widths` takes `standard14_widths` from code 0. Anything else is `FontWidths::None` at 1000 (inferred, read from `parse_widths`). The descriptor's `/MissingWidth` is parsed into `FontDescriptor::missing_width` but never used as that default (inferred: `git grep missing_width` finds no reader, 2026-10-30).
- **The Standard 14 tables are approximations.** Helvetica and Times have one table each that ignores the variant (`helvetica_widths(_variant)`, `times_widths(_variant)`), Courier is 600 throughout, and Symbol, ZapfDingbats and any other name are 500 for every code (inferred, read from `standard14.rs`; doc comments "Simplified Helvetica widths", "Simplified Times-Roman widths").
- `standard14_ascent_descent` gives the AFM ascender and descender magnitude for Courier, Helvetica and Times (629/157, 718/207, 683/217), `None` for Symbol and ZapfDingbats. Its callers are `FontEncoder` and form appearances (inferred, `git grep`).
- `is_standard14` and the tables strip a six-letter `ABCDEF+` subset tag before matching (`strip_subset_prefix`).
- On a `CIDFontType0`/`CIDFontType2` dictionary `parse_font_info` fills `FontWidths::CID` from `/W` and `/DW` ([CID fonts](cid-fonts.md)). A Type0 dictionary itself gets the simple-font path (no `/Widths`, so `FontWidths::None` unless its name is a Standard 14 one), and each consumer replaces it with the descendant's widths.
- `parse_font_descriptor` returns `None` without `/FontName`, so a descriptor that lacks it is dropped whole, `/FontBBox` and `/Flags` included (inferred).
- **Input bounds.** The dictionary comes from the file. Nothing here bounds the length of `/Widths` or `/W`; the object's own size, set by the parser, is the only bound (inferred).

## Code
- `justpdf-core/src/font/mod.rs` — `FontInfo`, `FontDescriptor`, `FontDescriptor::has_flag`, `FontDescriptor::is_symbolic`, `FontWidths`, `FontWidths::get_width`, `CIDWidthEntry`, `parse_font_info`, `resolve_font_entries`, `parse_font_descriptor`, `parse_widths`
- `justpdf-core/src/font/standard14.rs` — `is_standard14`, `standard14_widths`, `standard14_ascent_descent`, `strip_subset_prefix`, `helvetica_widths`, `times_widths`
- `justpdf-core/examples/list_fonts.rs` — calls `parse_font_info` on each page font
- `justpdf-core/tests/font_widths.rs` — `indirect_widths_place_the_glyphs`, `an_indirect_first_char_is_read`, `indirect_width_elements_are_read`, `a_widths_reference_that_does_not_resolve_is_ignored`
- `justpdf-render/tests/render_font_widths.rs` — `raster_glyphs_advance_by_indirect_widths`, `svg_glyphs_advance_by_indirect_widths`

## Reference behaviour
- MuPDF `source/pdf/pdf-font.c`: `pdf_array_get_*` resolve array elements (read raw 2026-10-08, #287); with no `/Widths`, every code 0–255 takes the font program's own advance, the substitute's for a non-embedded font (read raw 2026-10-08, cited on #292). justpdf takes the approximate table instead.
- The code cites "PDF spec 7.6" and "Table 123". Clauses to compare against, not yet compared: ISO 32000-2 §9.6 (simple fonts), §9.7 (composite fonts), §9.8 (descriptors).

## Cross-cutting invariants
- [Font resolution](../invariant/font-resolution.md) — the common starting point of the text and render paths.

## Blast radius
- [Content interpreter](content-interpreter.md), [Text extraction](text-extraction.md), [Glyph rendering](glyph-rendering.md), [SVG renderer](svg-renderer.md) — all place glyphs by these widths; a width that changes on one consumer's path and not another's moves extracted text away from the drawn glyph.
- [Redaction](redaction.md) — the glyph box it erases is the advance width this area gives by the `/FontBBox` height (ADR 0004); a width that is too small under-erases.
- [Font encodings](font-encodings.md) — `parse_encoding` fills `FontInfo::encoding` and `differences` from the dictionary this area resolves, and `FontEncoder` measures form text with these widths.
- [CID fonts](cid-fonts.md) — `parse_cid_widths` runs inside `parse_font_info`; both assume `resolve_font_entries` has resolved `/W` and `/DW`.
- [Font subsetting](font-subsetting.md) — relies on an unresolved `/Encoding` staying a reference, so it keeps the font whole rather than guess its glyphs.
- [Form appearance](form-appearance.md) — auto size and alignment use `standard14_widths` and `standard14_ascent_descent` for the default font.
- [Plaintext input](plaintext-input.md), [Format document](format-document.md) — plaintext, fb2 and mobi assume Courier's 600 width as `font_size * 0.6` instead of reading this table; changing Courier's widths here would not change their wrapping.
- [Font recovery](font-recovery.md) — `fallback_font_info` builds a `FontInfo` from `standard14_widths` of the substitute.

## Known holes / open
- The Standard 14 tables ignore the variant (bold and oblique take the regular widths), leave most of 0x80–0xFF at a default, and give Symbol and ZapfDingbats 500 everywhere. Since #227 the renderer draws these fonts with real URW glyphs, so the wrong widths show as uneven spacing (inferred). Tracked: #292.
- `/MissingWidth` is parsed and never used: a code outside `/FirstChar`…`/LastChar` takes 1000 (or 600) instead of the descriptor's missing width (inferred).
- A Type3 font's widths are glyph-space values that need `/FontMatrix`; this area returns them as they are and every consumer divides by 1000 ([Type3 fonts](type3-fonts.md)). Tracked: #312.
