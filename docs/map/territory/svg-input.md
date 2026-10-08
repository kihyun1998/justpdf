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
- [Render images](render-images.md) — until #229 justpdf's own renderer drew the resulting PDF blank: it decoded the Flate image XObject before `decode_image` decoded it again (measured 2026-09-30: a 200×100 SVG with a 180×80 red rect — pdfium drew 14400 red pixels, justpdf none; `decode_image` on the raw bytes succeeded, on the once-decoded bytes it returned "corrupt deflate stream").
- [SVG renderer](svg-renderer.md) — separate code that shares the name but goes the opposite way (PDF → SVG).

## Known holes / open
- `parse_length` reads `px` and `pt` as the same number of points, while `in`, `mm` and `cm` are converted to points; CSS has 1px = 0.75pt. Measured 2026-10-07 (`justpdf convert`): `width="96px"` and unitless `96` both give a 96 pt page. `parse_svg_dimensions` also takes the page from the `viewBox` whenever one is present, ignoring `width`/`height` (`width="10in" viewBox="0 0 100 50"` gives 100×50 pt), and no viewBox → viewport transform is applied to content. Decided in #197: px and unitless are both 0.75 pt, as CairoSVG does (`dpi=96`, `device_units_per_user_units` 0.75 for PDF); the page is `width`/`height`, with the viewBox fitted by `preserveAspectRatio`. Tracked: #197
- The resulting PDF renders blank in justpdf (Blast radius). Placement itself is right since #88 — pdfium draws the raster over the whole page.
- Tracked: #42 (render: the same double decode)
