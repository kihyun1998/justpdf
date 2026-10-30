# OCR (searchable PDF)

## What it is
Runs the external `tesseract` program on an image file or on a PDF page rendered to PNG, and builds a "searchable" PDF: each page's raster as the background with the recognised text drawn over it.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — of `justpdf-special`'s features only `ocr` pulls render (#1, closed 2026-05-08); the isolation script checks both sides.
- A rendered page reaches tesseract on stdin (`tesseract stdin stdout`), not through a temporary file — maintainer's call on #184 (2026-10-06). Shown: (a) `tempfile` as a dependency of `ocr`, (b) stdin, (c) a unique name built without a dependency (pid + counter + `create_new`), each with whether it collides, what it adds and whether the page touches the disk. Read for (b): `ProcessPagesInternal` in tesseract 3.04.00, 3.05.02, 4.1.0 and main takes `stdin`/`-` and sets stdin to binary on Windows, so tesseract older than 3.04 is not supported (3.03-rc1 has no stdin branch). The decision left out the swallowed OCR error in `make_searchable_pdf` and the `tesseract --version` spawn per call.

## Design model
Status: the stdin rule comes from #184; the rest is read from the source.
- `ocr_image` hands tesseract a path; `ocr_pdf_page` and `make_searchable_pdf` render the page to PNG and hand it over on stdin (`ocr_image_bytes`) and write no file. The PNG is written from a scoped thread while `wait_with_output` drains stdout and stderr, so neither pipe can fill while the other is waited on. Before #184 they wrote `temp_dir()/justpdf_ocr_{pid}[_{i}].png`, so concurrent calls in one process read each other's page or found it deleted: `test_concurrent_page_ocr_reads_its_own_page` failed 3 of 3 runs (`LIFE` read as `HEFT`; `cannot read input file`) and passed 3 of 3 after (measured on #184, tesseract 5.5.3).
- `ocr_image` and `ocr_pdf_page` first spawn `tesseract --version` (`is_tesseract_available`) on every call; `make_searchable_pdf` does not check, and when the spawn or the run fails the page gets an empty text layer (`unwrap_or_default`) with no error.
- Page image: `make_searchable_pdf` sizes each page from the crop box (else the media box), embeds the rendered raster with `embed_rgb` and draws it over the whole page with `draw_image` (`test_searchable_pdf_draws_the_page_image_as_an_image_object`, which runs without tesseract too, #88). Before #88 it built a placement `content_prefix` (`q w 0 0 h 0 0 cm`) and never used it, so the inline image sat at 1 pt × 1 pt.
- Text layer: the comment says "render mode 3 = invisible", but no `Tr` is written and `PageBuilder` has no operator for it. The text is drawn visibly in Helvetica at 1 pt: one `move_to(0, 1)`, then up to 100 lines as consecutive `Tj`, so they run on as one line along the bottom edge (inferred). Tesseract is asked for plain text, so there are no word boxes to place it by. The font is WinAnsi Helvetica, so recognised text beyond WinAnsi becomes `?` ([Content text encoding](../invariant/content-text-encoding.md)).
- Input bound: `dpi` is the caller's; nothing here bounds the rendered raster's size, which is decoded again with `image::load_from_memory` (inferred). The text layer is capped at 100 lines per page.

## Code
- `justpdf-special/src/ocr/mod.rs` — `is_tesseract_available`, `tesseract_version`, `ocr_image`, `ocr_image_bytes`, `tesseract_command`, `recognized_text`, `ocr_pdf_page`, `make_searchable_pdf`; tests `test_concurrent_page_ocr_reads_its_own_page`, `test_searchable_pdf_draws_the_page_image_as_an_image_object`, `test_tesseract_not_found_error`

## Reference behaviour
- tesseract's `ProcessPagesInternal` across 3.03-rc1 … main, read on #184 (2026-10-06) — settles the 3.04 minimum for stdin input.

## Cross-cutting invariants
- [Content text encoding](../invariant/content-text-encoding.md)

## Blast radius
- [Document builder](document-builder.md) — the layer can only become invisible and positioned once `PageBuilder` writes a text render mode, a text matrix and horizontal scaling; this module also assumes `move_to` is absolute.
- [Render API](render-api.md) — assumes `render_page` with `OutputFormat::Png` returns PNG bytes for page index `i` in `collect_pages` order.
- [Crate layering](crate-layering.md) — `ocr` is the only `justpdf-special` feature allowed to pull render.

## Known holes / open
- The layer is visible and unpositioned, and OCR failures in `make_searchable_pdf` are silent. Tracked: #179
- Recognised text beyond WinAnsi is written as `?`; a caller-supplied `Font` would fix it. Tracked: #322
- Each test asserts one side only: `test_tesseract_not_found_error` runs its check only without tesseract, `test_concurrent_page_ocr_reads_its_own_page` only with it. CI's all-features job installs tesseract and checks the `eng` data (#187), so the tesseract side runs there; the plain test job builds `justpdf-special` without `ocr`. Locally without tesseract both pass vacuously (measured 2026-10-30: 55 tests pass, `tesseract not found`).
- The concurrency test draws its words as filled-rectangle block capitals because, when it was written, the renderer drew non-embedded standard-14 text as boxes and `DocumentBuilder` could not attach an embedded font; since #173 it can (`add_font` + `set_font`, [CJK font embedding](cjk-font-embedding.md)), and the test was not changed.
