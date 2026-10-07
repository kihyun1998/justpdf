# Glyph rendering

## What it is
Turns the character codes of text-showing operators into glyph IDs, takes outlines from the embedded font and draws them, and caches the outlines.

## Governing decisions
**None.**

## Design model
- Font data is looked up in the order FontFile2 → FontFile3 → FontFile and all of it is handed to `ttf_parser::Face::parse`. Pure CFF and Type1 programs fail and a **placeholder box** is drawn (CFF measured 2026-10-07). Core's [CFF parser](cff.md) is not used. Decided in #224: a bare CFF goes through `ttf_parser::cff::Table` (glyph by name through the encoding, CID through a reverse of `glyph_cid`, scaled by `FontMatrix`), a path #227's bundled substitutes reuse.
- Code → GID for a simple font: the first of `font::truetype_glyph_candidates(face, code, encoding, differences, symbolic)` returns a simple TrueType code's candidate GIDs, most preferred first: the §9.6.6.4 choice pdf.js makes (`src/core/fonts.js`, read raw 2026-10-07) — one cmap subtable (non-symbolic: (3,1), else (1,0); symbolic: (3,0), else (1,0); a (0,x) Unicode table is a candidate too); for a non-symbolic font with an `/Encoding` and a (3,1) or (1,0) table, the code's glyph name (`/Differences`, else a MacRoman/WinAnsi base, else Standard) read as Unicode through the AGL or as its MacRoman code; for (3,0), the code in `0xF0xx`; otherwise the code itself; then `post` by name (`/Differences` or a MacRoman/WinAnsi base name; forced first for a (0,x) table) — then fallbacks: every subtable with the code and its `0xF000`/`0xF100`/`0xF200` prefixes, the names through any Unicode subtable and `post`, the code's WinAnsi character, and the code as a GID. `encoding` is `None` when the font has no `/Encoding`: then even (3,1) is read with the raw code, as pdf.js does. One function, two callers — maintainer's call in #223 (2026-10-07). The code → glyph name tables (`STANDARD_GLYPH_NAMES`, `MAC_ROMAN_GLYPH_NAMES`, `WIN_ANSI_GLYPH_NAMES`) come from the same generator as the Annex D tables. The renderer resolves an indirect `/Encoding`, reads `symbolic` from the font descriptor (direct or indirect, bit 3 of `/Flags`), and computes the glyphs once per string. No candidate draws nothing (measured, `tests/render_encoding.rs`). Until #223 the byte was looked up as a Unicode scalar, then taken as a GID: `/Differences [65 /B]` drew exactly "A" and MacRoman `8E` did not draw é (measured 2026-10-07, NotoSans pixel hashes); `glyph::char_code_to_glyph_id` is removed.
- When the face parses and a glyph is chosen but has no outline (a space), nothing is drawn. Until #223 that fell through to the placeholder box: one space at 60 pt was 1,728 dark pixels (measured). The box remains only for a font program `ttf_parser` cannot read (#224, #225) and for no embedded program (#227).
- Where the encoding's glyph name has no glyph, the first candidate is a fallback (the raw code in another subtable, or the code as a GID), so a glyph is drawn where pdf.js draws nothing; `/Differences [65 /g123]` on Noto draws "A" (measured). Maintainer's call in #223 (2026-10-07): keep drawing the first candidate, fallbacks included, chosen over stopping after the §9.6.6.4 part as pdf.js does — a file with a broken encoding still shows something.
- `justpdf-core` re-exports `ttf_parser` and the renderer uses it from there; the renderer has no `ttf-parser` dependency of its own, so there is one version.
- CID fonts use `/CIDToGIDMap` but not `/W` — [Font resolution](../invariant/font-resolution.md).
- Text is not filled with patterns (`fill_color_rgba` only — inferred).
- The cache key is `glyph_cache::font_hash` + GID, with a default capacity of 4096. `font_hash` is FNV-1a over the font program's length and all of its bytes, computed once when the font is resolved and kept beside the program in `ResolvedFont::font_program` (inferred). It is not keyed by the address of the font data: `render_text_string` clones that data on every call and drops it, so one address names different fonts over time — keyed that way, alternating two same-size fonts drew the other font's glyph on 99–100 of 200 lookups (measured 2026-10-02). Nor by a prefix: two programs can agree in length and in their first 256 bytes and differ only further in (`justpdf-render/tests/render_glyphs.rs` builds such a pair from Noto Sans, measured). Byte-identical programs share entries, which is sound because the outline depends only on the program and the GID (inferred). Hashing takes 0.9 ms for Noto Sans (431 KB) and 27 ms for 20 MiB, against 1.4 ms and 69 ms for Flate-decoding the same bytes (measured 2026-10-06, release build, Apple silicon); both run once per font for each page rendered, since every page gets its own `RenderInterpreter` (inferred). The maintainer kept the whole-program hash after seeing these numbers (2026-10-06, a judgement, not a derivation); hashing and decoding once per document instead of once per page is the open direction. #163. Decided in #240: the per-document cache lives on core `PdfDocument` (decoded bytes as `Arc<[u8]>` plus the hash, keyed by the font-file stream reference, failures cached, cleared on `authenticate`), so `render_page`, the CLI page loop and parallel rendering all share it without an API change; `font_hash` moves into core, and the renderer stops cloning the program per text operator and parses `Face` once per text operator.

## Code
- `justpdf-render/src/interpreter.rs` — `select_font`, `resolve_font`, `extract_font_data`, `get_font_descriptor`, `render_text_string`, `render_glyph`, `adjust_text_position`
- `justpdf-render/src/glyph.rs` — `glyph_outline`, `units_per_em`
- `justpdf-core/src/font/glyph_select.rs` — `truetype_glyph_candidates`, `preferred_subtable`
- `justpdf-render/tests/render_encoding.rs` — `differences_choose_the_named_glyph`, `mac_roman_code_draws_its_mac_roman_glyph`, `a_glyph_without_an_outline_draws_nothing`
- `justpdf-render/src/glyph_cache.rs` — `GlyphCache`

## Reference behaviour
**None.** Clauses to compare against: ISO 32000-2 §9.6.6 (glyph selection in simple fonts), §9.7.4.

## Cross-cutting invariants
- [Font resolution](../invariant/font-resolution.md)

## Blast radius
- [Font loading](font-loading.md), [CID fonts](cid-fonts.md), [Font encodings](font-encodings.md) — inputs (partly bypassed).
- [Font subsetting](font-subsetting.md) — keeps the union of the same candidates, and a subset font keeps its GIDs, so the renderer's first candidate reaches the same glyph as in the original.
- [Font recovery](font-recovery.md), [CFF parser](cff.md) — unconnected modules that could be used instead of the placeholder box.
- [SVG renderer](svg-renderer.md) — a separate text path.

## Known holes / open
- Text with no embedded font (#227), CFF (#224), Type1 (#225) and Type3 (#226) are drawn as boxes (inferred; no pixel-checking test). Standard-14 Helvetica rendered as one black box per glyph (measured 2026-10-06, `render_page` at 150 dpi, #184). The box is already `/Widths` wide, so a substitute keeps today's positions. Decided in #227: bundle MuPDF's URW base-14 set (`resources/fonts/urw/*.cff`, SIL OFL 1.1, about 610 KB) behind a default-on feature, chosen with `find_substitute` and drawn by glyph name; blocked by #224 (#223 landed). The `.otf`/`.ttf` set in `urw-base35-fonts` is AGPLv3 and not usable here; pdf.js ships Foxit `.pfb` (BSD) plus Liberation Sans 1.07 (GPLv2 + font exception).
- Tracked: #222 (CID widths), #240 (per-document font program cache)
