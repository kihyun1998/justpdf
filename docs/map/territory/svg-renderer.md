# SVG renderer

## What it is
Outputs a page as an SVG document. It is not a device but a **second interpreter**: it has its own operator dispatch, font resolution, content assembly, XObject, image and ExtGState handling, and shares with the raster side only the `graphics_state` types, `ResourceScopes` and the `optional_content` helpers.

## Governing decisions
**None.** [ADR-0006](../../adr/0006-breaking-changes-are-allowed-before-1-0.md) keeps `svg_device` a public module; it does not decide its behaviour.

## Design model
Read from the source and the issues cited; no record states it.

- **What it leaves out** (re-verified, `SvgRenderer::execute_op`): inline images (`BI`) and shadings (`sh`, commented "skip for now") are skipped; pattern names are recorded in the state but never used, so a pattern fill paints in the fill colour; soft masks (`/SMask` in an ExtGState) and transparency groups are not read; annotations are not drawn (neither `render_page_to_svg` nor `svg_device.rs` reads `/Annots`). The `defs` field comment mentions "gradients" but there is no gradient code.
- **Every page starts with a white `<rect>`** the page's size (`build_svg`); there is no background option.
- Resource names resolve through the same scope stack as the raster side (`ResourceScopes`) — [resource name scope](../invariant/resource-scope.md).
- A Form XObject is skipped when a running form (`running_forms`) is met again, and when `xobject_depth > 10` — [Content stream recursion](../invariant/content-stream-recursion.md).
- **Text becomes `<text>` elements, not glyph outlines** (`render_text_string`). The character is the ToUnicode mapping of the code; without ToUnicode, the code itself when it is below 128. A code with neither draws a rectangle placeholder `/Widths` wide. The element carries a position, a `font-family` from `/BaseFont` and a size taken from the matrix's y scale, so rotated or skewed text is drawn upright (inferred, `render_text_string`). A simple font's `/Encoding` is not read for this. Tracked: #297
- Its `resolve_xobject` passes image streams raw to `decode_image`, as the raster side does (#229). Until #229 it decoded every non-DCT image first, and a Flate image XObject produced no `<image>` element (measured 2026-10-06, #229).
- **An image is drawn as decoded colour only** (`render_image`): an image's own `/SMask` or `/Mask` is not applied, and an `/ImageMask` stencil goes through the same path instead of painting the fill colour (inferred, `render_image` has no mask branch).
- Its `image_to_rgba` adds alpha to the core `DecodedImage::to_rgb8` (#46) — [Image pixel layout](../invariant/image-pixel-layout.md).
- Its `cs`/`CS` resolve colour spaces like the raster interpreter's (`colour_space`, #231). Until #231 its own `cs_from_name` fell back to DeviceGray for any other name, so even a resource naming `/DeviceRGB` filled black (measured 2026-10-06, #231).
- `gs` applies `/Font [font size]` through `select_font_ref`, the same rule as the raster interpreter ([Render interpreter](render-interpreter.md), #238), plus `LW`, `LC`, `LJ`, `ML`, `ca`, `CA` and `BM` (re-verified, `apply_extgstate`). Until #238 the font was ignored: with no prior `Tf` the text emitted no `<text>`; after `/F1 12 Tf` (Helvetica, width 500) it stayed Helvetica with advance 6 instead of Courier's 7.2; a `gs` font absent from `/Font` (Times-Roman 20) was drawn as the previous Helvetica 12 (measured 2026-10-07, #238).
- Optional content OFF in the default configuration is not emitted, by the same decision as the raster side (`optional_content`, #218) — [optional content](optional-content.md). `MP`, `DP`, `BMC` and tags other than `/OC` stay no-ops.
- **A clip is one `<clipPath>` per `W`, and it replaces the one before** (`apply_clip` sets `active_clip_id` to the new path, which does not reference the old one). `q`/`Q` save and restore the id. So nested clips are not intersected: `q 0 0 100 200 re W n 50 0 150 200 re W n … re f Q` emitted the fill with only the second clip, painting x 100–200 (measured 2026-10-30 with the CLI at `88b2a08`).
- **`W` takes effect at once**, so the painting operator in the same path is clipped by it: `re W S` emitted the stroke with `clip-path` of its own rectangle (measured 2026-10-30). The raster side does the same ([Render clipping](render-clipping.md)).
- `render_form_xobject` floors the state stack like the raster interpreter (#128): an unmatched `Q` in a form cannot pop the caller's state, and the stack and clip-id stack are unwound to their depth on entry afterwards. Until #128 a form with content `q` moved what followed by the form's `/Matrix` (measured, #128: the next fill's transform `matrix(1,0,0,-1,50,200)` where `…,0,200)` is right). Pinned by `svg_*` in `tests/render_stream_floor.rs`.
- What bounds the input: the same as the raster interpreter's content (nothing in the renderer), and each image is embedded as a base64 PNG of its full decoded size, with no cap (inferred, `render_image`).

## Code
- `justpdf-render/src/svg_device.rs` — `SvgRenderer`, `SvgRenderer::new`, `SvgRenderer::render_page`, `build_svg`, `execute_ops`, `execute_op`, `select_font`, `select_font_ref`, `resolve_font`, `get_page_content`, `concat_content_streams`, `apply_clip`, `stroke_attrs`, `fill_current_path`, `stroke_current_path`, `fill_and_stroke_path`, `do_xobject`, `resolve_xobject`, `render_image`, `render_form_xobject`, `apply_extgstate`, `image_to_rgba`, `colour_space`, `render_text_string`
- Tests calling `render_page_to_svg` (command: `rg -l render_page_to_svg justpdf-render/tests`): `render_cid_cmap.rs`, `render_cid_widths.rs`, `render_colour_spaces.rs`, `render_extgstate_font.rs`, `render_font_widths.rs`, `render_image_colour.rs`, `render_images.rs`, `render_optional_content.rs`, `render_recursion.rs`, `render_resources.rs`, `render_rotation.rs`, `render_stream_floor.rs`, `render_test.rs`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Image pixel layout](../invariant/image-pixel-layout.md)
- [Page content assembly](../invariant/page-content-assembly.md)
- [Font resolution](../invariant/font-resolution.md)
- [Content stream recursion](../invariant/content-stream-recursion.md)
- [resource name scope](../invariant/resource-scope.md)
- [Hidden content keeps its state](../invariant/hidden-content-state.md)
- [Clip timing](../invariant/clip-timing.md)

## Blast radius
- [Render interpreter](render-interpreter.md) — the same operators are dispatched there; a fix made on the raster side does not reach this copy.
- [Optional content](optional-content.md) — `optional_content` decides `BDC` sections and XObject `/OC` here as on the raster side.
- [Image decoding](image-decoding.md), [ToUnicode](tounicode.md), [Color spaces](color-spaces.md) — the decoded pixel layout, the Unicode it writes as text, and the colour conversion are taken from them unchanged.
- [Render API](render-api.md) — `render_page_to_svg` sizes the document from `page_frame`.
- [CLI](cli.md) — `render -F svg` and `convert` hand this output to users as a file.

## Known holes / open
- **Every stroked path is malformed XML**: `stroke_current_path` and `fill_and_stroke_path` write `stroke_attrs` and the `transform` attribute with no space between them (`stroke-miterlimit="10"transform=…`). A page with `20 w … re S` gave an SVG that Python's `xml.dom.minidom` rejects as "not well-formed (invalid token)" (measured 2026-10-30 at `88b2a08`). Tracked: #417.
- Nested clips are not intersected, and `W` clips its own painting operator (measured, above). Tracked: #418.
- A Form XObject drawn by `Do` is not clipped to its `/BBox` (inferred, `render_form_xobject`). The raster renderer clips it since #365.
- Shadings, inline images, soft masks, transparency groups, image masks and annotations are not drawn (above).
- Text without ToUnicode ignores the font's `/Encoding`. Tracked: #297
- Vertical writing is drawn as a horizontal run. Tracked: #324
