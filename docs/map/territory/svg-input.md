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
- [Render images](render-images.md) — justpdf's own renderer draws the resulting PDF blank: `resolve_xobject` decodes the Flate image XObject, then `decode_image` decodes it again with the same dictionary, fails, and `do_xobject` drops the error (measured 2026-09-30: a 200×100 SVG with a 180×80 red rect — pdfium draws 14400 red pixels, justpdf none; `decode_image` on the raw bytes succeeds, on the once-decoded bytes it returns "corrupt deflate stream").
- [SVG renderer](svg-renderer.md) — separate code that shares the name but goes the opposite way (PDF → SVG).

## Known holes / open
- The resulting PDF renders blank in justpdf (Blast radius). Placement itself is right since #88 — pdfium draws the raster over the whole page.
- Tracked: #42 (render: the same double decode)
