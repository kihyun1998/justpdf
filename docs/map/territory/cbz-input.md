# CBZ input

## What it is
Turns the images in a comic-book ZIP archive into PDF pages, one image per page, in natural filename order.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — `cbz` (`zip`) builds without render (#2).

## Design model
Status: read from the source only.
- Entries: every non-directory whose name ends in `.jpg`, `.jpeg`, `.png`, `.gif`, `.bmp` or `.webp`, sorted by `natural_sort_key` (digit runs padded to 20 places, letters lower-cased). An entry whose dimensions cannot be read is dropped silently. The `image` dependency is built with `png` and `jpeg` only, so GIF, BMP and WebP pages are dropped this way (inferred from `Cargo.toml` and `image_dimensions`). No valid image is an error.
- Page size: the image's pixel size in points (1 px = 1 pt).
- `to_pdf` decodes each image to RGB with the `image` crate (alpha dropped), embeds it as a Flate DeviceRGB image XObject with `embed_rgb`, and draws it over the page with `draw_image` (`test_cbz_to_pdf_draws_each_image_as_an_image_object` checks the `q cm Do Q` operators, the `cm` operands and the decoded pixels, #88). Before #88 it called `draw_inline_image` without a `cm`, so the image sat at 1 pt × 1 pt — with a real page PNG the render came out all white (poppler, 2026-09-25).
- justpdf's renderer draws the result (measured 2026-10-30 at 88b2a08: a one-PNG CBZ converted and rendered back with `justpdf convert` shows the image).
- Preview: decodes the image and resizes it with Lanczos3 to the DPI; `page_text` is always empty.
- Input bound: none. Every image entry is read whole into memory at `from_bytes`, and each is fully decoded again by `to_pdf` and by every preview; nothing limits entry count, compressed or decoded size (inferred).

## Code
- `justpdf-formats/src/cbz/mod.rs` — `CbzDocument`, `CbzDocument::open`, `CbzDocument::from_bytes`, `is_image_extension`, `image_dimensions`, `natural_sort_key`, `impl FormatDocument for CbzDocument` (`render_page`, `render_page_png`, `to_pdf`)

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Document builder](document-builder.md) — assumes `embed_rgb` takes three bytes per pixel, rows from the top, and that `draw_image` maps the unit square to the page box.
- [Render images](render-images.md) — a producer of Flate DeviceRGB image XObjects that justpdf's renderer must decode exactly once (see [SVG input](svg-input.md) for the blank-page history before #229).
- [Format document](format-document.md) — the contract.

## Known holes / open
- GIF, BMP and WebP entries are accepted by name and then dropped without a message (inferred).
- No bound on entry count or size (Design model).
