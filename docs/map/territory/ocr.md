# OCR (searchable PDF)

## What it is
OCRs images and PDF pages with the external `tesseract` program and builds a searchable PDF with an invisible text layer over the scanned image. A PDF page is turned into a PNG with render and then OCRed.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — only the `ocr` feature pulls in render (#1).
- Maintainer decision (2026-10-06, #184): a rendered page reaches tesseract on stdin (`tesseract stdin stdout`), not through a temporary file. Shown: (a) `tempfile` as a dependency of the `ocr` feature, (b) stdin, (c) a unique name built without a dependency (pid + counter + `create_new`); for each, whether it collides, what it adds as a dependency, and whether the page image touches the disk. Read for (b): `ProcessPagesInternal` in tesseract 3.04.00, 3.05.02, 4.1.0 and main takes `stdin`/`-` and sets stdin to binary on Windows — so tesseract older than 3.04 is not supported (3.03-rc1 has no stdin branch; 3.04.00 has it). Chosen: (b). Not covered: the swallowed OCR error in `make_searchable_pdf`, and the `tesseract --version` spawn per page.

## Design model
- `make_searchable_pdf` puts the rendered page image in as an image XObject with `embed_rgb` and draws it over the whole page with `draw_image` (`test_searchable_pdf_draws_the_page_image_as_an_image_object` — it runs without tesseract too, only the text layer is empty; #88). Before #88 it built a placement `content_prefix` (`q w 0 0 h 0 0 cm`) and never used it, so the inline image landed at 1pt×1pt.
- The comment says "render mode 3 = invisible", but no `Tr` is written, and `PageBuilder` has no way to do it either. The "invisible" OCR text shows up as 1pt Helvetica piled in one spot (inferred). Non-ASCII shares the [Content text encoding](../invariant/content-text-encoding.md) problem.
- `ocr_image` hands tesseract a path; `ocr_pdf_page` and `make_searchable_pdf` hand it the rendered PNG on stdin (`ocr_image_bytes`) and write no file. The PNG is written from a second thread while `wait_with_output` drains stdout and stderr, so neither pipe can fill while the other is waited on. Before #184 they wrote `temp_dir()/justpdf_ocr_{pid}[_{i}].png`, so concurrent calls in one process read each other's page or found it already deleted: `test_concurrent_page_ocr_reads_its_own_page` failed 3 of 3 runs (`LIFE` read as `HEFT`; `cannot read input file`), and passes 3 of 3 since (measured, tesseract 5.5.3).
- `make_searchable_pdf` does not check for tesseract; when it is missing the spawn fails and the page gets an empty text layer (`unwrap_or_default`).

## Code
- `justpdf-special/src/ocr/mod.rs` — `is_tesseract_available`, `tesseract_version`, `ocr_image`, `ocr_image_bytes`, `ocr_pdf_page`, `make_searchable_pdf`, tests `test_concurrent_page_ocr_reads_its_own_page`, `test_searchable_pdf_draws_the_page_image_as_an_image_object`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Content text encoding](../invariant/content-text-encoding.md)

## Blast radius
- [Document builder](document-builder.md) — it has to support the text render mode and an image `cm` for this feature to do what it promises.
- [Render API](render-api.md) — page rasterization.
- [Crate layering](crate-layering.md) — the `ocr` feature gate.

## Known holes / open
- The tests assert only one side each, with tesseract present or absent (`test_tesseract_not_found_error` only when absent, `test_concurrent_page_ocr_reads_its_own_page` only when present). The CI all-features job installs tesseract, so there the tesseract-backed side runs (#187); the plain test job has no `ocr` feature.
- The concurrency test draws its words as filled-rectangle block capitals because the renderer draws non-embedded standard-14 text as boxes and the public `DocumentBuilder` API cannot attach an embedded TrueType font to a page ([Document builder](document-builder.md), #65).
- The text layer goes through `show_text`, so recognised text is WinAnsi with `?` beyond it (inferred; covered by the `show_text` tests, #34).
- Tracked: #173 (text beyond WinAnsi)
