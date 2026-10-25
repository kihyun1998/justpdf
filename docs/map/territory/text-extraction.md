# Text extraction

## What it is
Runs the [Content interpreter](content-interpreter.md) over page content for glyph positions (text state, font widths), turns characters into Unicode (ToUnicode → encoding), groups them into words and lines, then reorders them by [Reading order](reading-order.md). CLI `text`, the facade's `Page::text` and every binding's `extract_*_text` are this pipeline.

## Governing decisions
**None.**

## Design model
- Text state, glyph advance and font loading are the [Content interpreter](content-interpreter.md)'s (#207); this side turns each glyph into a `TextChar` (`CharCollector`): Unicode, the origin of the text rendering matrix, its y-scale as the size, and the width in text space (`w0 / 1000 × Tfs × Th`, not through the CTM).
- Does font resolution **separately** from the renderer: the interpreter's `load_font` reads `/W`, and this side builds Unicode from ToUnicode and the encoding — [Font resolution](../invariant/font-resolution.md).
- An ExtGState's `/Font [font size]` applied with `gs` sets the font and size as `Tf` would (#235); the [Content interpreter](content-interpreter.md) holds the rule. A character shown in such a font has an empty `TextChar::font_name`, since no resource name selected it. Measured 2026-10-06 before the fix by glyph width: with `/GS1` setting `/Font [<Courier> 12]`, `/GS1 gs (ABC) Tj` measured 18.00 (fallback) and `/F1 12 Tf … /GS1 gs … (ABC) Tj` kept Helvetica's 24.67, against Courier's 21.60; Xpdf 4.00 `pdftotext` switches font. `tests/extgstate_font.rs` now measures 21.60 for both (36.00 at 20 pt).
- **Does not enter forms**: text inside Form XObjects is not extracted. The interpreter can (`InterpretOptions::enter_forms`); extraction leaves it off (#48). The renderer recurses.
- **Leaves out hidden optional content by default** (#219, maintainer decision at triage: a caller option, defaulting to the document's default configuration). `CharCollector` drops a glyph the [Content interpreter](content-interpreter.md) flags `hidden` unless `TextExtractOptions::include_hidden`; the interpreter still reports it, so redaction can erase it (ADR 0004). The four original entry points keep their signatures and use the default; `extract_page_text_with_options` and `extract_all_text_with_options` take the options. Until #219 hidden text was extracted (measured 2026-10-06: `(HiddenLayer) Tj` inside `/OC /L BDC … EMC`, group `/OFF` in `/D`, extracted by `justpdf-cli text`; Xpdf 4.00 `pdftotext` leaves it out). Pinned by `justpdf-core/tests/text_optional_content.rs`. CLI and bindings expose no option (out of scope for #219).
- Concatenates page content with its own private function — [Page content assembly](../invariant/page-content-assembly.md).
- Joins pages with a blank line (`"\n\n"`) (`extract_all_text_string`).

## Code
- `justpdf-core/src/text/mod.rs` — `TextExtractOptions`, `extract_page_text`, `extract_page_text_with_options`, `extract_all_text`, `extract_all_text_with_options`, `extract_page_text_string`, `extract_all_text_string`, `get_page_content_data`, `TextInterpreter`, `CharCollector`, `group_into_words`, `group_into_lines`, `PageText`

## Reference behaviour
**None.** Clauses to compare against: ISO 32000-2 §9.3, §9.10. No record of comparing extraction results with MuPDF (example) — `justpdf-core/examples/compare_mupdf.rs` is a performance comparison.

## Cross-cutting invariants
- [Font resolution](../invariant/font-resolution.md)
- [Page content assembly](../invariant/page-content-assembly.md)

## Blast radius
- [Content interpreter](content-interpreter.md) — positions, widths and fonts.
- [Font loading](font-loading.md), [Font encodings](font-encodings.md), [ToUnicode](tounicode.md), [CID fonts](cid-fonts.md) — input.
- [Content stream parsing](content-stream-parsing.md) — operator input.
- [Reading order](reading-order.md), [Text output formats](text-output-formats.md), [Text search](text-search.md) — consumers of this output (`PageText`). When changing the struct, check all three.
- [Font subsetting](font-subsetting.md) — used as the judge in compress tests.
- [Optional content](optional-content.md) — hidden layers left out by default.
- [CLI](cli.md), [Facade](facade.md), [Language bindings](language-bindings.md) — consumers of the public extraction API.

## Known holes / open
- Text inside Form XObjects is not extracted (measured 2026-10-06: a page showing `(OnPage) Tj` and drawing a form that shows `(InsideForm) Tj` extracts as `OnPage` with `justpdf-cli text`; Xpdf 4.00 `pdftotext` gives `OnPage InsideForm`).
- Tracked: #48 (Form XObject text). Once it lands, a form whose `/OC` is hidden contributes no text by default: the interpreter already flags its glyphs (#219).
