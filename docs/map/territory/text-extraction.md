# Text extraction

## What it is
Turns a page's content into positioned text: it concatenates the page's content streams, runs the [Content interpreter](content-interpreter.md) for glyph positions, turns each glyph into a `TextChar` with Unicode (ToUnicode, then the font's encoding), groups characters into words and words into lines, and hands the lines to [Reading order](reading-order.md) for blocks. The result is `PageText`. CLI `text`, the facade's `Document::text`, `Page::text` and `Page::text_structured`, every binding's text call, and the compression tests' judge are this pipeline.

## Governing decisions
**None.** [ADR 0004](../../adr/0004-redaction-never-under-erases.md) moved glyph positioning out of this area into the shared interpreter so that redaction erases what extraction reports; it governs the [Content interpreter](content-interpreter.md), not the grouping here. [ADR 0005](../../adr/0005-sanitizing-removes-unpainted-text-by-category.md) names this area's visibility rule (the default configuration) as the one sanitizing must match.

## Design model
From reading the source, the maintainer decision cited, and the tests named.
- **Glyphs to characters** (`CharCollector`, #207): Unicode is the ToUnicode entry for the code, else `decode_simple_code` for a one-byte code, else `decode_text` over the bytes; with no font, the bytes as lossy UTF-8. A glyph whose Unicode is empty is dropped. `TextChar::x`/`y` are the origin of the text rendering matrix and `font_size` its y-scale, so they include `Tm` and the CTM; `TextChar::width` is `w0 / 1000 × Tfs × Th` in text space, so it does not (#318). The field's doc comment says "user space".
- **Font resolution is separate from the renderer's**: the interpreter's `load_font` reads widths, and Unicode comes from ToUnicode and the encoding here — [Font resolution](../invariant/font-resolution.md). A Type0 font without ToUnicode yields its raw bytes (#328).
- **`gs` with an ExtGState `/Font`** sets font and size as `Tf` would (#235); the interpreter holds the rule. A character shown in such a font has an empty `TextChar::font_name`. Measured 2026-10-06 before the fix, by glyph width: with `/GS1` setting `/Font [<Courier> 12]`, `/GS1 gs (ABC) Tj` measured 18.00 (fallback widths) and `/F1 12 Tf … /GS1 gs … (ABC) Tj` kept Helvetica's 24.67, against Courier's 21.60; Xpdf 4.00 `pdftotext` switches font (#235). `tests/extgstate_font.rs` now expects 21.60, and 36.00 at 20 pt.
- **Forms are not entered**: the interpreter runs with `InterpretOptions::default()`, so text inside Form XObjects is not extracted (#48); the renderers do enter them.
- **Hidden optional content is left out by default** (#219; maintainer decision at triage: a caller option, defaulting to the document's default configuration). `CharCollector` drops a glyph the interpreter flags `hidden` unless `TextExtractOptions::include_hidden`; the interpreter still reports it, so redaction can erase it (ADR 0004). `extract_page_text` and `extract_all_text` use the default; `extract_page_text_with_options` and `extract_all_text_with_options` take the options. Until #219 hidden text was extracted (measured 2026-10-06: `(HiddenLayer) Tj` inside `/OC /L BDC … EMC` with the group `/OFF` in `/D` came out of `justpdf-cli text`; Xpdf 4.00 `pdftotext` leaves it out). Pinned by `tests/text_optional_content.rs`. CLI and bindings expose no option. Invisible text (`Tr 3`) is extracted: nothing reads the rendering mode here (inferred).
- **Words**: a new word starts where the gap from the previous character's end exceeds 0.3 × its font size, where the baseline moves by more than 0.5 × font size, or at a space character (`group_into_words`). Producers that write no space glyphs and a narrower inter-word gap (pdfTeX) extract with no spaces (#298); a font sized through `Tm` makes every letter a word, because `width` is in text space (#318).
- **Lines**: words are sorted top to bottom, then left to right, and a new line starts where a word's baseline differs from the line's by more than 0.5 × its font size (`group_into_lines`). The grouping spans the whole page width, so words of two columns on one baseline become one line before column detection sees them ([Reading order](reading-order.md)).
- **Content assembly**: `get_page_content_data` concatenates the page's streams with a space between them, its own private function — [Page content assembly](../invariant/page-content-assembly.md).
- **Joins**: `PageText::plain_text` joins blocks with a blank line, and `extract_all_text_string` joins pages with a blank line.
- **Bounds on input**: one page's decoded content is held whole (decoded size is [Stream filters](stream-filters.md)' question, #161); the number of characters, words and lines is not bounded; grouping is a sort plus linear passes.

## Code
- `justpdf-core/src/text/mod.rs` — `TextChar`, `TextWord`, `TextLine`, `TextBlock`, `PageText`, `PageText::plain_text`, `TextInterpreter`, `CharCollector`, `group_into_words`, `group_into_lines`, `build_line`, `get_page_content_data`, `decode_content_obj`, `TextExtractOptions`, `extract_page_text`, `extract_page_text_with_options`, `extract_all_text`, `extract_all_text_with_options`, `extract_page_text_string`, `extract_all_text_string`
- `justpdf-core/tests/text_optional_content.rs` — `a_hidden_layer_is_left_out_unless_asked_for`, `hidden_text_still_moves_the_text_position`

## Reference behaviour
**None** as a recorded comparison of extraction output with MuPDF (the binding example reference). `justpdf-core/examples/compare_mupdf.rs` compares speed, not text. Spot comparisons with Xpdf 4.00 `pdftotext` are cited above (#219, #235, #48). Clauses to compare against: ISO 32000-2 §9.3, §9.10.

## Cross-cutting invariants
- [Font resolution](../invariant/font-resolution.md) — `CharCollector` (Unicode).
- [Page content assembly](../invariant/page-content-assembly.md) — `get_page_content_data`.

## Blast radius
- [Content interpreter](content-interpreter.md) — positions, widths, fonts and the `hidden` flag come from there; a unit changed there (text space versus user space) breaks the gap thresholds here.
- [Font loading](font-loading.md), [Font encodings](font-encodings.md), [ToUnicode](tounicode.md), [CID fonts](cid-fonts.md) — the Unicode of every character; a decoding change changes every output and search.
- [Content stream parsing](content-stream-parsing.md) — the operation list the interpreter walks.
- [Reading order](reading-order.md), [Text output formats](text-output-formats.md), [Text search](text-search.md) — read `PageText`; `TextChar::width`, `TextLine::x` and the line grouping are their inputs, so changing a field's meaning or the grouping changes all three.
- [Optional content](optional-content.md) — what counts as hidden; the renderers' judgement of the same content must match (ADR 0005).
- [Font subsetting](font-subsetting.md), [Compress pipeline](compress-pipeline.md) — compression tests use extracted text as the judge that a rewritten page still reads the same; an extraction change can turn those tests red without a compression bug.
- [CLI](cli.md), [Facade](facade.md), [Language bindings](language-bindings.md) — expose `extract_*`; a changed default (hidden content, forms) changes their output with no flag to restore it.

## Known holes / open
- Text inside Form XObjects is not extracted (measured 2026-10-06: a page showing `(OnPage) Tj` and drawing a form that shows `(InsideForm) Tj` extracts as `OnPage` with `justpdf-cli text`; Xpdf 4.00 `pdftotext` gives `OnPage InsideForm`). Once it is, a form whose `/OC` is hidden contributes no text by default, since the interpreter already flags its glyphs. Tracked: #48.
- `TextChar::width` ignores `Tm` and the CTM. Tracked: #318.
- No spaces in pdfTeX text. Tracked: #298.
- A Type0 font without ToUnicode extracts raw bytes. Tracked: #328.
- Vertical writing is extracted as horizontal runs. Tracked: #313, #323.
- An inline `/OC` property list is extracted though the renderers hide it (measured, see [Content interpreter](content-interpreter.md)).
