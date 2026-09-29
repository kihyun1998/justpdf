# SVG input

## What it is
Draws SVG with its own pixel rasterizer, then puts the result into the PDF as an image (not as vectors).

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — builds without render.

## Design model
- Curves are drawn as straight lines ("Simplified"), text becomes "a small marker", and percentage lengths are skipped.
- `to_pdf` renders a raster at 72dpi, puts it in as an image XObject (Flate) with `embed_rgb`, and draws it over the whole page with `draw_image` (`test_to_pdf_draws_the_raster_as_an_image_object`, #88). Before #88 it called `draw_inline_image` without a `cm`, so the image was placed at 1pt×1pt, and pixels containing space + `EI` + space cut the image short.

## Code
- `justpdf-formats/src/svg/mod.rs` — `SvgDocument`, `render_element`, `to_pdf`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Document builder](document-builder.md) — `embed_rgb`, `draw_image`.
- [Render images](render-images.md) — the resulting PDF renders as a blank page in justpdf itself.
- [SVG renderer](svg-renderer.md) — separate code that shares the name but goes the opposite way (PDF → SVG).

## Known holes / open
- Image placement in the resulting PDF (above).
