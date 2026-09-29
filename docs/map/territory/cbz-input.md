# CBZ input

## What it is
Turns the images in a comic-book ZIP archive into pages, in natural sort order.

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md)

## Design model
- Decodes to RGB with the `image` crate, makes an image XObject (Flate) with `embed_rgb`, and draws it over the whole page (sized to the image's pixels) with `draw_image` (`test_cbz_to_pdf_draws_each_image_as_an_image_object`, #88). Before #88 it called `draw_inline_image` without a `cm`, so the image was placed at 1pt×1pt — with a real page PNG as input the render came out all white (poppler, 2026-09-25).

## Code
- `justpdf-formats/src/cbz/mod.rs` — `CbzDocument`, `to_pdf`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Document builder](document-builder.md) — `embed_rgb`, `draw_image`. [Render images](render-images.md) — the image XObject path.

## Known holes / open
- `test_cbz_to_pdf` only checks that the output starts with `%PDF`.
