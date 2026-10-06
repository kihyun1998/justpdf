# Glyph rendering

## What it is
Turns the character codes of text-showing operators into glyph IDs, takes outlines from the embedded font and draws them, and caches the outlines.

## Governing decisions
**None.**

## Design model
- Font data is looked up in the order FontFile2 → FontFile3 → FontFile and all of it is handed to `ttf_parser::Face::parse`. Pure CFF and Type1 programs fail and a **placeholder box** is drawn (inferred). Core's [CFF parser](cff.md) is not used.
- Code → GID: `char_code_to_glyph_id` treats the byte as a Unicode scalar and looks it up in the font's cmap, and if that fails, code == GID. The font's `/Encoding` and `/Differences` are not used ([Font encodings](font-encodings.md)).
- CID fonts use `/CIDToGIDMap` but not `/W` — [Font resolution](../invariant/font-resolution.md).
- Text is not filled with patterns (`fill_color_rgba` only — inferred).
- The cache key is the font's FNV hash + GID, with a default capacity of 4096.

## Code
- `justpdf-render/src/interpreter.rs` — `select_font`, `resolve_font`, `extract_font_data`, `get_font_descriptor`, `render_text_string`, `render_glyph`, `adjust_text_position`
- `justpdf-render/src/glyph.rs` — `glyph_outline`, `char_code_to_glyph_id`, `units_per_em`
- `justpdf-render/src/glyph_cache.rs` — `GlyphCache`

## Reference behaviour
**None.** Clauses to compare against: ISO 32000-2 §9.6.6 (glyph selection in simple fonts), §9.7.4.

## Cross-cutting invariants
- [Font resolution](../invariant/font-resolution.md)

## Blast radius
- [Font loading](font-loading.md), [CID fonts](cid-fonts.md), [Font encodings](font-encodings.md) — inputs (partly bypassed).
- [Font subsetting](font-subsetting.md) — a subset font keeps its GIDs, so both the cmap lookup and the code == GID fallback in `char_code_to_glyph_id` reach the same glyph as in the original (subsetting keeps the glyphs of both paths).
- [Font recovery](font-recovery.md), [CFF parser](cff.md) — unconnected modules that could be used instead of the placeholder box.
- [SVG renderer](svg-renderer.md) — a separate text path.

## Known holes / open
- `GlyphCache::font_hash` caches a font's hash by the address of the font data, and `render_text_string` clones and drops that data on every call, so a later font reusing the address gets the earlier font's glyph outlines; which glyphs are affected depends on allocation patterns (measured 2026-09-30: two builds differing only in unrelated code drew different glyphs on four pages, identical once the memo was removed). Tracked: #163
- Text with no embedded font (#227), CFF (#224), Type1 (#225) and Type3 (#226) are drawn as boxes (inferred; no pixel-checking test). Standard-14 Helvetica rendered as one black box per glyph (measured 2026-10-06, `render_page` at 150 dpi, #184).
- Tracked: #222 (CID widths), #223 (encoding in glyph selection)
