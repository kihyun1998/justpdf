# Office input (DOCX/XLSX/PPTX)

## What it is
Reads the text of Office Open XML documents — DOCX paragraphs, XLSX cells, PPTX slide text — and writes it into a PDF as Courier lines.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — `office` (`zip`, `roxmltree`) builds without render (#2).
- The preview rule is the one in [EPUB input](epub-input.md): an error without `plaintext`, maintainer's call on #35 (2026-10-02).

## Design model
Status: read from the source; the preview rule comes from #35.
- Type: `OfficeDocument::open` takes it from the extension (`docx`, `xlsx`, `pptx`, else `UnsupportedFormat`); `from_bytes` takes it as an argument.
- DOCX: one `FormatDocument` page for the whole document — every `w:p` in `word/document.xml`, with `w:t`, `w:tab` and `w:br` inside `w:r` and inside `hyperlink`; other run content (fields, text boxes, tables' structure) is not read specially (inferred).
- XLSX: one page per `xl/worksheets/sheet*.xml`, sorted as strings, so `sheet10` comes before `sheet2` (inferred); cells tab-separated by column from `c@r`, shared strings resolved.
- PPTX: one page per `ppt/slides/slide*.xml`, sorted by slide number; `a:t` text joined per `a:p`.
- `to_pdf`: one PDF page per `FormatDocument` page, US Letter, 72 pt margins, 12 pt lines; it stops at the bottom margin ("Would need pagination here for real use"), so a DOCX longer than one page loses the rest. No wrapping. Each line goes through `move_to` ([Format document](format-document.md)).
- Preview: as EPUB — `PlainTextDocument` page 0 under `cfg(feature = "plaintext")`, else `FormatError::preview_needs_plaintext()` after the index check. `office` compiles alone (measured 2026-10-30), with the `wrap_fixed_pitch` dead-code warning.
- Input bound: none. Parts are read whole from the ZIP; `column_index_from_ref` multiplies by 26 per letter in `u32`, so a cell reference of seven or more letters overflows (a panic in a debug build), and the row is then allocated as `max_col + 1` strings, up to about 4 billion (inferred).

## Code
- `justpdf-formats/src/office/mod.rs` — `OfficeType`, `OfficeDocument`, `OfficeDocument::open`, `OfficeDocument::from_bytes`, `extract_office_metadata`, `extract_docx_text`, `extract_docx_paragraph_text`, `extract_xlsx_text`, `parse_shared_strings`, `parse_xlsx_sheet`, `column_index_from_ref`, `extract_pptx_text`, `extract_slide_number`, `impl FormatDocument for OfficeDocument` (`render_page`, `render_page_png`, `to_pdf`)

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Content text encoding](../invariant/content-text-encoding.md)

## Blast radius
- [Plaintext input](plaintext-input.md) — the preview is `PlainTextDocument`'s page 0.
- [Crate layering](crate-layering.md) — `office` must stay render-free and compile alone.
- [Format detection](format-detection.md) — `open` keeps its own extension list, which must agree with `detect_format`'s.
- [Format document](format-document.md) — the contract.
- [Document builder](document-builder.md) — assumes `show_text` writes WinAnsi for Courier and that `move_to` is absolute.

## Known holes / open
- No page breaks: text past one page is dropped (Design model).
- XLSX sheets are ordered as strings, not by number (inferred).
- A long cell reference overflows `u32` and drives an unbounded allocation (inferred).
- Wrapping and caller-supplied fonts. Tracked: #174
