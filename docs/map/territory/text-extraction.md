# Text extraction

## What it is
Interprets page content to compute glyph positions (text state, font widths), turns characters into Unicode (ToUnicode → encoding), groups them into words and lines, then reorders them by [Reading order](reading-order.md). CLI `text`, the facade's `Page::text` and every binding's `extract_*_text` are this pipeline.

## Governing decisions
**None.**

## Design model
- Text state and glyph advance follow the formulas of spec §9.3 (cited in code comments).
- Does font resolution **separately** from the renderer: this side reads `/W` and builds Unicode from ToUnicode and the encoding — [Font resolution](../invariant/font-resolution.md).
- Does not resolve `/Font` in ExtGState (`gs`) ("We don't resolve ExtGState for now" — it has no document access) (#235).
- **Does not handle `Do`**: text inside Form XObjects is not extracted. The renderer recurses.
- Does not look at BDC/OC (optional content) — text in hidden layers is extracted too (#219).
- Concatenates page content with its own private function — [Page content assembly](../invariant/page-content-assembly.md).
- Joins pages with a blank line (`"\n\n"`) (`extract_all_text_string`).

## Code
- `justpdf-core/src/text/mod.rs` — `extract_page_text`, `extract_all_text`, `extract_page_text_string`, `extract_all_text_string`, `resolve_fonts`, `resolve_to_unicode`, `get_page_content_data`, `TextInterpreter`, `show_string`, `show_tj_array`, `group_into_words`, `group_into_lines`, `PageText`

## Reference behaviour
**None.** Clauses to compare against: ISO 32000-2 §9.3, §9.10. No record of comparing extraction results with MuPDF (example) — `justpdf-core/examples/compare_mupdf.rs` is a performance comparison.

## Cross-cutting invariants
- [Font resolution](../invariant/font-resolution.md)
- [Page content assembly](../invariant/page-content-assembly.md)

## Blast radius
- [Font loading](font-loading.md), [Font encodings](font-encodings.md), [ToUnicode](tounicode.md), [CID fonts](cid-fonts.md) — input.
- [Content stream parsing](content-stream-parsing.md) — operator input.
- [Reading order](reading-order.md), [Text output formats](text-output-formats.md), [Text search](text-search.md) — consumers of this output (`PageText`). When changing the struct, check all three.
- [Font subsetting](font-subsetting.md) — used as the judge in compress tests.
- [Optional content](optional-content.md) — hidden layers ignored.
- [CLI](cli.md), [Facade](facade.md), [Language bindings](language-bindings.md) — consumers of the public extraction API.

## Known holes / open
- Text inside Form XObjects is not extracted (measured 2026-10-06: a page showing `(OnPage) Tj` and drawing a form that shows `(InsideForm) Tj` extracts as `OnPage` with `justpdf-cli text`; Poppler `pdftotext` gives `OnPage InsideForm`).
- Tracked: #48 (Form XObject text, blocked by #207), #235 (ExtGState `/Font`, blocked by #207), #219 (hidden optional-content text)
