# XPS input

## What it is
Reads the `UnicodeString` text of `Glyphs` elements from the FixedDocumentSequence of an XPS/OpenXPS package and writes it into a PDF as Courier lines. Layout, images and vector paths are not carried over.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — the `xps` feature (`zip`, `roxmltree`) builds without render; the isolation script checks it (#2).

## Design model
Status: read from the source only.
- Finding the pages: `_rels/.rels` relationship whose type contains `fixeddocumentsequence`, else three known paths, else the first `.fdseq` in the archive; then `DocumentReference@Source` → `PageContent@Source`. A package with no pages is an error.
- Page size: `FixedPage@Width`/`Height` in 1/96 inch, converted to points; 816 × 1056 (US Letter) when absent.
- Text: each non-empty trimmed `UnicodeString`, one per line in document order. `OriginX`/`OriginY` are ignored, so the PDF does not keep positions.
- `to_pdf` starts at the top margin (72 pt) and stops at the bottom margin (`break`): text past one page is dropped, there is no wrapping, and each line goes through `move_to`, so only the first line is visible ([Format document](format-document.md), inferred for XPS from the shared loop, measured for plaintext and FB2).
- `render_page` returns a white page of the right size ("v1: return white page… Full rendering of XPS paths/glyphs would go here").
- Input bound: none. Each part is read whole from the ZIP, every page's XML is kept in memory (`XpsPage::xml`, unused), and `render_page` allocates `w * h * 4` in `u32` from the page's declared size and the DPI, which overflows on a large page (inferred; the same expression panics in a debug build for SVG — [SVG input](svg-input.md)).

## Code
- `justpdf-formats/src/xps/mod.rs` — `XpsDocument`, `XpsDocument::open`, `XpsDocument::from_bytes`, `find_fixed_doc_sequence`, `parse_fixed_doc_sequence`, `parse_fixed_document`, `parse_fixed_page`, `extract_glyphs_text`, `extract_metadata`, `resolve_xps_path`, `impl FormatDocument for XpsDocument` (`render_page`, `to_pdf`)

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Content text encoding](../invariant/content-text-encoding.md)

## Blast radius
- [Format document](format-document.md) — the contract this implements.
- [Document builder](document-builder.md) — assumes `show_text` writes WinAnsi for Courier and that `move_to` is absolute.

## Known holes / open
- The preview is a blank page.
- Only the first line of each page is visible, and text past one page is dropped (Design model).
- No bound on part size or page size (Design model).
- Wrapping and caller-supplied fonts. Tracked: #174
