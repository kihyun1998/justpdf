# OCR (searchable PDF)

## What it is
OCRs images and PDF pages with the external `tesseract` program and builds a searchable PDF with an invisible text layer over the scanned image. A PDF page is turned into a PNG with render and then OCRed.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — only the `ocr` feature pulls in render (#1).

## Design model
- `make_searchable_pdf` puts the rendered page image in as an image XObject with `embed_rgb` and draws it over the whole page with `draw_image` (`test_searchable_pdf_draws_the_page_image_as_an_image_object` — it runs without tesseract too, only the text layer is empty; #88). Before #88 it built a placement `content_prefix` (`q w 0 0 h 0 0 cm`) and never used it, so the inline image landed at 1pt×1pt.
- The comment says "render mode 3 = invisible", but no `Tr` is written, and `PageBuilder` has no way to do it either. The "invisible" OCR text shows up as 1pt Helvetica piled in one spot (inferred). Non-ASCII shares the [Content text encoding](../invariant/content-text-encoding.md) problem.
- It talks to tesseract through temporary files.

## Code
- `justpdf-special/src/ocr/mod.rs` — `is_tesseract_available`, `tesseract_version`, `ocr_image`, `ocr_pdf_page`, `make_searchable_pdf`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Content text encoding](../invariant/content-text-encoding.md)

## Blast radius
- [Document builder](document-builder.md) — it has to support the text render mode and an image `cm` for this feature to do what it promises.
- [Render API](render-api.md) — page rasterization.
- [Crate layering](crate-layering.md) — the `ocr` feature gate.

## Known holes / open
- The tests assert only one side each, with tesseract present or absent (`test_tesseract_not_found_error` only when absent).
- Tracked: #34 (non-ASCII content text)
