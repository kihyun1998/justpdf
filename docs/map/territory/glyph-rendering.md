# Glyph rendering

## What it is
Turns the character codes of text-showing operators into glyph IDs, takes outlines from the embedded font and draws them, and caches the outlines.

## Governing decisions
**None.**

## Design model
- Font data is looked up in the order FontFile2 → FontFile3 → FontFile and all of it is handed to `ttf_parser::Face::parse`. Pure CFF and Type1 programs fail and a **placeholder box** is drawn (CFF measured 2026-10-07). Core's [CFF parser](cff.md) is not used. Decided in #224: a bare CFF goes through `ttf_parser::cff::Table` (glyph by name through the encoding, CID through a reverse of `glyph_cid`, scaled by `FontMatrix`), a path #227's bundled substitutes reuse.
- Code → GID: `char_code_to_glyph_id` treats the byte as a Unicode scalar and looks it up in the font's cmap, and if that fails, code == GID. The font's `/Encoding` and `/Differences` are not used ([Font encodings](font-encodings.md)).
- CID fonts use `/CIDToGIDMap` but not `/W` — [Font resolution](../invariant/font-resolution.md).
- Text is not filled with patterns (`fill_color_rgba` only — inferred).
- The cache key is `glyph_cache::font_hash` + GID, with a default capacity of 4096. `font_hash` is FNV-1a over the font program's length and all of its bytes, computed once when the font is resolved and kept beside the program in `ResolvedFont::font_program` (inferred). It is not keyed by the address of the font data: `render_text_string` clones that data on every call and drops it, so one address names different fonts over time — keyed that way, alternating two same-size fonts drew the other font's glyph on 99–100 of 200 lookups (measured 2026-10-02). Nor by a prefix: two programs can agree in length and in their first 256 bytes and differ only further in (`justpdf-render/tests/render_glyphs.rs` builds such a pair from Noto Sans, measured). Byte-identical programs share entries, which is sound because the outline depends only on the program and the GID (inferred). Hashing takes 0.9 ms for Noto Sans (431 KB) and 27 ms for 20 MiB, against 1.4 ms and 69 ms for Flate-decoding the same bytes (measured 2026-10-06, release build, Apple silicon); both run once per font for each page rendered, since every page gets its own `RenderInterpreter` (inferred). The maintainer kept the whole-program hash after seeing these numbers (2026-10-06, a judgement, not a derivation); hashing and decoding once per document instead of once per page is the open direction. #163

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
- Text with no embedded font (#227), CFF (#224), Type1 (#225) and Type3 (#226) are drawn as boxes (inferred; no pixel-checking test). Standard-14 Helvetica rendered as one black box per glyph (measured 2026-10-06, `render_page` at 150 dpi, #184). The box is already `/Widths` wide, so a substitute keeps today's positions. Decided in #227: bundle MuPDF's URW base-14 set (`resources/fonts/urw/*.cff`, SIL OFL 1.1, about 610 KB) behind a default-on feature, chosen with `find_substitute` and drawn by glyph name; blocked by #224 and #223. The `.otf`/`.ttf` set in `urw-base35-fonts` is AGPLv3 and not usable here; pdf.js ships Foxit `.pfb` (BSD) plus Liberation Sans 1.07 (GPLv2 + font exception).
- Tracked: #222 (CID widths), #223 (encoding in glyph selection)
