# Font resolution paths

## The fact
The same font **must be interpreted the same way** by text extraction and by rendering: splitting character codes (1/2 bytes), code → glyph mapping, code → Unicode mapping, glyph widths. If the two paths compute widths differently, the positions of extracted text and of rendered glyphs drift apart, and search highlights, redaction areas and layout analysis stop matching the screen.

## Why it is cross-cutting
Both consumers start from core `parse_font_info`, but each builds on top of it **on its own**, and neither calls the other:
- Text reads `/W` and `/DW` (render does not) and decodes with the encoding tables (render does not use them).
- Render reads `/CIDToGIDMap` (text does not need it), and picks a simple TrueType glyph through the font's encoding with `font::truetype_glyph_candidates`, the function subsetting also uses (#223).
- There are two copies of the ToUnicode interpretation code.
- Core has several font modules that neither path uses (CFF, Type3, recovery, OpenType layout).

## Territories it holds in
- [Font loading](../territory/font-loading.md) — the common starting point (does not fill in CID widths).
- [Font encodings](../territory/font-encodings.md) — encoding tables only text uses.
- [ToUnicode](../territory/tounicode.md) — two copies of the interpretation.
- [CID fonts](../territory/cid-fonts.md) — widths only in text, GID mapping only in render.
- [Text extraction](../territory/text-extraction.md) — `resolve_fonts`, `resolve_to_unicode`.
- [Render interpreter](../territory/render-interpreter.md) — `resolve_font`.
- [Glyph rendering](../territory/glyph-rendering.md) — the first of `truetype_glyph_candidates`.
- [SVG renderer](../territory/svg-renderer.md) — the third `resolve_font`.
- [Font subsetting](../territory/font-subsetting.md) — the two paths read the subset result differently (render through the font cmap, text through ToUnicode). Subsetting keeps the union of the same simple-TrueType candidates the renderer draws the first of (`simple_font_glyph_ids`), and resolves CID fonts on its own (`cid_font_glyph_ids`); it keeps GIDs, so it loses no glyphs.

## What a violation looks like
- On a page with a Type0 (CJK) font, the rendered character spacing and the extracted text coordinates differ (render uses a fixed width of 1000 — inferred).
- A subset font loses glyphs or draws the wrong ones in render while extracted text is fine — compress tests that only look at extraction pass.
- In a `/Differences` font, extraction (#221), simple-TrueType render (#223), bare-CFF render (#224) and Type1 render (#225) follow the encoding.

## Discovery history
No incident recorded. Reported on 2026-09-23 by the font/text and render research agents while the map was being written. All of it was inferred from reading code.

- 2026-09-23, while working on #9, the subset case was **reproduced by running it** (simple TrueType): the subset font's `cmap` pointed at old GIDs so all five characters to be drawn lost their outlines, while text extraction on the same output was unchanged — [Font subsetting](../territory/font-subsetting.md#design-model).

- Tracked: #222 (CID widths in render)
- 2026-10-08, while proving #225 on arXiv 1706.03762, an indirect `/Widths` was **measured** to fall back to the default width on both sides (render) and inferred for text: `parse_font_info` cannot resolve references, and each consumer patches indirect `/Encoding` on its own (SVG does not). Decided in #287: a shared core font-loading step with a resolver — the first piece of the shared interpretation this invariant asks for; #222 builds on it. Tracked: #287.

## Where it will recur
**Adding, on either the text or the render side, a function that gets a code, glyph, width or Unicode value from a font dictionary is subject to this invariant.** Check: does the other side need the same information? Until core has a shared font interpretation type, a fix on one side has to explicitly decide whether the other side needs the same fix.
