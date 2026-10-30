# Type3 fonts

## What it is
`font::type3` parses a Type3 font dictionary (`/FontBBox`, `/FontMatrix`, `/CharProcs`, `/Encoding /Differences`, `/FirstChar`, `/LastChar`, `/Widths`, `/Resources`) into a `Type3Font`, and maps a character code to its glyph name and CharProc stream. Nothing in product code uses it: the renderers draw a Type3 glyph as a box and never run its CharProc, and text extraction reads a Type3 font through the general [Font loading](font-loading.md) path.

## Governing decisions
**None.** No decision record.

## Design model
Status: from reading the source; no record.

- `parse_type3_font` returns `None` without a four-element `/FontBBox` or a direct `/CharProcs` dictionary. `/FontMatrix` defaults to `DEFAULT_FONT_MATRIX` `[0.001 0 0 0.001 0 0]` unless it has six elements (inferred).
- `/Differences` is read with the general simple-font parser: `parse_encoding_differences` calls `font::parse_differences` (#221). `glyph_name` takes the **first** entry for a code (`find`), while `FontInfo::decode_simple_code` takes the last ([Font encodings](font-encodings.md)) (inferred).
- The function takes a bare `PdfDict` and does not resolve references: an indirect `/Encoding`, `/CharProcs`, `/Widths` or `/FontMatrix` reads as absent, and a `/CharProcs` entry that is not a reference is dropped (inferred).
- `/FirstChar` and `/LastChar` are cast to `u8` (`as u8`), so a value above 255 wraps (inferred). `get_width` returns 0 outside the range.
- **Widths are glyph-space values.** Every consumer reads a Type3 font's widths through `parse_font_info` and divides by 1000; none applies `/FontMatrix` (inferred, read from `content/interpret.rs` and both renderers; issue #312 reports the same from code). The render test fixture uses `/FontMatrix [0.001 0 0 0.001 0 0]`, so no test catches a different matrix (`render_font_widths.rs`).
- **Input bound.** The dictionary comes from the file; nothing here bounds the number of CharProcs or `/Differences` entries beyond the object's size (inferred).

## Code
- `justpdf-core/src/font/type3.rs` — `parse_type3_font`, `Type3Font`, `Type3Font::get_width`, `Type3Font::glyph_name`, `Type3Font::char_proc`, `Type3Font::resolve_char_proc`, `parse_encoding_differences`, `parse_font_matrix`, `parse_char_procs`, `DEFAULT_FONT_MATRIX`
- `justpdf-render/src/interpreter.rs` — the `d0`/`d1` arm of the operator match (no-ops)
- `justpdf-render/tests/render_font_widths.rs` — `raster_glyphs_advance_by_indirect_widths` (a Type3 font with no CharProcs, drawn as boxes)

## Reference behaviour
**None.** Not compared against a reference. Clause to compare against: ISO 32000-2 §9.6.4.

## Cross-cutting invariants
- [Font resolution](../invariant/font-resolution.md) — Type3 widths are read the same wrong way on the text and render paths, so they agree with each other but not with the font.

## Blast radius
- [Render interpreter](render-interpreter.md), [Glyph rendering](glyph-rendering.md) — `d0`/`d1` are no-ops and CharProcs are not run; Type3 rendering would start from `Type3Font::resolve_char_proc`. Both assume widths in 1/1000 units, which a non-default `/FontMatrix` breaks.
- [Content interpreter](content-interpreter.md), [Text extraction](text-extraction.md) — divide widths and `/FontBBox` by 1000 and ignore `/FontMatrix`; positions are wrong for a font with another matrix.
- [Redaction](redaction.md) — the glyph box comes from the same widths and `/FontBBox`; with a larger `/FontMatrix` it is too small and under-erases (ADR 0004; #312).
- [Font encodings](font-encodings.md) — owns `parse_differences`; a change there changes Type3 glyph names.

## Known holes / open
- No consumer of `Type3Font` in product code (measured, `git grep parse_type3_font` at 88b2a08).
- Type3 glyphs are not drawn: CharProcs never run. Tracked: #226.
- Widths and `/FontBBox` are read as 1/1000 units instead of through `/FontMatrix`. Tracked: #312.
- `parse_type3_font` does not resolve indirect entries, so a future consumer must resolve them first (inferred).
