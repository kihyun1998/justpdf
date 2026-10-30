# Font resolution paths

## The fact
The same font **must be interpreted the same way** by text extraction and by rendering: splitting character codes (1/2 bytes), code → glyph mapping, code → Unicode mapping, glyph widths. If the two paths compute widths differently, the positions of extracted text and of rendered glyphs drift apart, and search highlights, redaction areas and layout analysis stop matching the screen.

## Why it is cross-cutting
Both consumers start from core `parse_font_info`, but each builds on top of it **on its own**, and neither calls the other:
- Text decodes with the encoding tables (render does not use them). CID widths (`/W`, `/DW`) are shared since #222.
- `/CIDToGIDMap` is read at three sites that do not call each other: the renderer (`RenderInterpreter::parse_cid_to_gid_map`, `parse_cid_gid_stream`), font subsetting (`cid_font_glyph_ids` in `writer/compress.rs`) and `font/encoder.rs` (`cid_to_gid_map`, added by #178 for appearance text); the encoder's result must name the glyph the renderer draws (re-read 2026-10-30 at 88b2a08; inferred). Text does not need it. Render picks a simple TrueType glyph through the font's encoding with `font::truetype_glyph_candidates`, the function subsetting also uses (#223).
- ToUnicode has one parser (`ToUnicodeCMap::parse`, `font/cmap.rs`), but each consumer finds and decodes the `/ToUnicode` stream on its own: `resolve_to_unicode` in `content/interpret.rs`, `font/encoder.rs`, and the renderers' `resolve_font` (re-read 2026-10-30 at 88b2a08; the old map's "two copies of the interpretation code" no longer holds; inferred).
- Of core's other font modules, render uses `font::recovery` (`find_substitute` in `interpreter.rs`, and `substitute.rs`); bare CFF programs are read through `justpdf_core::ttf_parser::cff` with core's `cff_simple_glyph`/`cff_cid_to_gid` helpers, not through `font::cff` (`glyph.rs`). Text uses neither. `font::cff`, Type3 and OpenType layout are used by neither path (`git grep` at 88b2a08; measured).

Advances follow the same split. The advance rules — word spacing `Tw` on single-byte code 32 only, Type3 widths through `/FontMatrix`, vertical writing mode — are applied separately in `RenderInterpreter::render_text_string`, `SvgRenderer::render_text_string` and the core content interpreter, and each is wrong the same way at all three today. Tracked: #314, #312, #313, #323, #324 (reported by the 2026-10-30 rewrite; inferred).

## Territories it holds in
- [Font loading](../territory/font-loading.md) — the common starting point (does not fill in CID widths).
- [Font encodings](../territory/font-encodings.md) — encoding tables only text uses.
- [ToUnicode](../territory/tounicode.md) — one parser, located and decoded separately per consumer.
- [CID fonts](../territory/cid-fonts.md) — widths shared (#222); `/CIDToGIDMap` read separately by render, subsetting and the encoder.
- [Type3 fonts](../territory/type3-fonts.md) — `/FontMatrix` is ignored alike by text extraction and both renderers, so widths agree with each other but not with the font. Tracked: #312.
- [Text extraction](../territory/text-extraction.md) — `CharCollector` (Unicode).
- [Content interpreter](../territory/content-interpreter.md) — `load_font` (widths, descriptor, ToUnicode).
- [Render interpreter](../territory/render-interpreter.md) — `resolve_font`.
- [Glyph rendering](../territory/glyph-rendering.md) — the first of `truetype_glyph_candidates`.
- [SVG renderer](../territory/svg-renderer.md) — the third `resolve_font`.
- [Font subsetting](../territory/font-subsetting.md) — the two paths read the subset result differently (render through the font cmap, text through ToUnicode). Subsetting keeps the union of the same simple-TrueType candidates the renderer draws the first of (`simple_font_glyph_ids`), and resolves CID fonts on its own (`cid_font_glyph_ids`); it keeps GIDs, so it loses no glyphs.

## What a violation looks like
- On a page with a Type0 (CJK) font, the rendered character spacing and the extracted text coordinates differ (before #222, render used a fixed width of 1000 for Type0 fonts — inferred at the time; #222 moved CID widths into core).
- A subset font loses glyphs or draws the wrong ones in render while extracted text is fine — compress tests that only look at extraction pass.
- In a `/Differences` font, extraction (#221), simple-TrueType render (#223), bare-CFF render (#224) and Type1 render (#225) follow the encoding.

## Discovery history
No incident recorded. Reported on 2026-09-23 by the font/text and render research agents while the map was being written. All of it was inferred from reading code.

- 2026-09-23, while working on #9, the subset case was **reproduced by running it** (simple TrueType): the subset font's `cmap` pointed at old GIDs so all five characters to be drawn lost their outlines, while text extraction on the same output was unchanged — [Font subsetting](../territory/font-subsetting.md#design-model).

- 2026-10-08, #222: CID widths moved into core (`parse_font_info` on the descendant), so text extraction and both renderers advance a Type0 font alike; a glyph with `/W` 2000 at 10 pt renders 20 units wide, as it extracts.
- 2026-10-08, while proving #225 on arXiv 1706.03762, an indirect `/Widths` was **measured** to fall back to the default width on both sides (render) and inferred for text: `parse_font_info` cannot resolve references, and each consumer patches indirect `/Encoding` on its own (SVG does not). #287 put a shared step in core: `font::resolve_font_entries` resolves `/Widths`, `/FirstChar`, `/LastChar` and `/Encoding` for text extraction, the raster and SVG renderers and font subsetting — the first piece of the shared interpretation this invariant asks for; #222 builds on it.

## Where it will recur
**Adding, on either the text or the render side, a function that gets a code, glyph, width or Unicode value from a font dictionary is subject to this invariant.** Check: does the other side need the same information? Until core has a shared font interpretation type, a fix on one side has to explicitly decide whether the other side needs the same fix.
