# SVG input

## What it is
Parses an SVG with roxmltree, draws it with its own small pixel rasterizer, and puts the raster into a one-page PDF as an image — not as vectors. It is unrelated to the [SVG renderer](svg-renderer.md), which shares the name and goes the other way (PDF → SVG).

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) — `svg` (`roxmltree`) builds without render (#2).
- Units and viewport, decided on #197 (maintainer, 2026-10-07), not built: px and unitless lengths are both 0.75 pt, as CairoSVG does (`dpi=96`, `device_units_per_user_units` 0.75 for PDF); the page is `width`/`height`, with the `viewBox` fitted by `preserveAspectRatio` (default `xMidYMid meet`).

## Design model
Status: read from the source; the units rule is a decision not yet in the code.
- Elements: `rect`, `circle`, `ellipse`, `line`, `polyline`, `polygon`, `path`, `text`, with `g` recursion and `transform` composition (`parse_node`). Paths are stroked as straight segments: curves go straight to their end point ("Simplified"), and paths are never filled. Text becomes a 4 × 8 px marker in the fill colour ("we just place a small marker"); its characters still reach `page_text`. Percentage lengths parse to nothing ("percentages need parent context, skip").
- Size: `parse_svg_dimensions` takes the page from the `viewBox` whenever it has four numbers, ignoring `width`/`height` and the viewBox origin, else from `width`/`height` (default 300 × 150). `parse_length` reads `px`, `pt` and unitless as the same number of points; `in`, `mm`, `cm` convert to points. Measured on #197 (2026-10-07, `justpdf convert`): `width="96px"` and unitless `96` both give a 96 pt page; `width="10in" viewBox="0 0 100 50"` gives 100 × 50 pt. No viewBox → viewport transform is applied to content (inferred on #197).
- `to_pdf` renders at 72 dpi (one pixel per point), drops alpha, embeds the raster as a Flate DeviceRGB image XObject with `embed_rgb`, and draws it over the whole page with `draw_image` (`test_to_pdf_draws_the_raster_as_an_image_object`, #88). Before #88 it called `draw_inline_image` without a `cm`, so the image sat at 1 pt × 1 pt, and pixel bytes holding space + `EI` + space cut the image short.
- justpdf's own renderer draws the result. Measured 2026-10-30 at 88b2a08: a 200 × 100 SVG with a 180 × 80 red rect, converted and rendered back with `justpdf convert`, shows the rect in justpdf and in pdftoppm. Until #229 (closed 2026-10-08) it drew the page blank: the Flate stream was decoded once and `decode_image` decoded it again (measured 2026-09-30: pdfium drew 14400 red pixels, justpdf none; `decode_image` on the once-decoded bytes returned "corrupt deflate stream").
- Input bound: none. `open` reads the file whole as UTF-8; `render_rgba` allocates `w * h * 4` computed in `u32` from the declared size, so a large size overflows. Measured 2026-10-30 (debug build): `width="70000" height="70000"` panics with `attempt to multiply with overflow` at `render_rgba`; in a build without overflow checks the product wraps and the buffer is the wrong size (inferred). A size just under the overflow allocates up to 4 GiB (inferred). Nesting: `parse_node` and `collect_text_content` read at most 256 levels below the root (`MAX_XML_DEPTH`), skipping deeper elements and text, and roxmltree 0.21.2 parses without recursing, so a 100,000-deep SVG converts (#449; [XML nesting depth](../invariant/xml-nesting-depth.md)).

## Code
- `justpdf-formats/src/svg/mod.rs` — `SvgDocument`, `SvgDocument::open`, `SvgDocument::from_bytes`, `SvgDocument::from_string`, `SvgDocument::render_rgba`, `parse_svg_dimensions`, `parse_length`, `parse_style`, `parse_color`, `parse_transform`, `parse_node`, `parse_path_data`, `render_element`, `fill_rect_pixels`, `fill_ellipse_pixels`, `draw_line_pixels`, `fill_polygon_pixels`, `blend_pixel`, `impl FormatDocument for SvgDocument` (`render_page`, `to_pdf`)

## Reference behaviour
- CairoSVG `cairosvg/helpers.py` (`UNITS`, `size()`) and `surface.py`, read on #197 (2026-10-07) — settles the unit and viewport rule above.

## Cross-cutting invariants
- [XML nesting depth](../invariant/xml-nesting-depth.md)

## Blast radius
- [Document builder](document-builder.md) — assumes `embed_rgb` takes three bytes per pixel, rows from the top, and that `draw_image` maps the unit square to the page box.
- [Render images](render-images.md) — this is a producer of Flate DeviceRGB image XObjects that justpdf's renderer must decode exactly once; a regression there blanks these pages again (as before #229).
- [Format detection](format-detection.md) — `.svgz` is detected as SVG, which this module cannot read (it does not gunzip).

## Known holes / open
- Units and viewport: px read as points, viewBox replaces `width`/`height`, no viewBox transform on content. Tracked: #197
- Curves, fills of paths, text and percentages are approximated or skipped (Design model).
- A large declared size overflows the pixel-buffer size (Design model). Tracked: #426
- `.svgz` is not read ([Format detection](format-detection.md)).
