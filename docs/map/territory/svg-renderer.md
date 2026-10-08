# SVG renderer

## What it is
Outputs a page as an SVG document. It is not a device but a **second interpreter**: it has its own operator dispatch, font resolution, content assembly, XObject, image and ExtGState handling, and shares only the `graphics_state` types with the raster side.

## Governing decisions
**None.**

## Design model
- Inline images (`BI`) are skipped, shading (`sh`) is "skip for now", marked content is ignored entirely (no OCG), and pattern names are only recorded, never used. No annotations, no soft masks. The `defs` comment mentions "gradients" but there is no gradient code.
- Resource names resolve through the same scope stack as the raster side (`ResourceScopes`) — [resource name scope](../invariant/resource-scope.md).
- A Form XObject is skipped when a running form (`running_forms`) is met again, and skipped past depth 10 — [Content stream recursion](../invariant/content-stream-recursion.md).
- Text becomes `<text>` through ToUnicode; without it only ASCII (<128); without an outline, a "rectangle placeholder".
- Its `resolve_xobject` decodes every non-DCT image before `render_image` hands it to `decode_image`, the same double decode as the raster side: a Flate image XObject produces no `<image>` element (measured 2026-10-06). Tracked: #42.
- It has copies of `image_to_rgba` and `cs_from_name` — [Image pixel layout](../invariant/image-pixel-layout.md).
- `gs` applies an ExtGState's `/Font [font size]` through `select_font_ref`, the same rule as the raster interpreter ([Render interpreter](render-interpreter.md), #238). Until #238 it was ignored: with no prior `Tf` the text emitted no `<text>`; after `/F1 12 Tf` (Helvetica, width 500) it stayed Helvetica with advance 6 instead of Courier's 7.2; a `gs` font absent from `/Font` (Times-Roman 20) was drawn as the previous Helvetica 12 (measured 2026-10-07).

## Code
- `justpdf-render/src/svg_device.rs` — `SvgRenderer`, `execute_op`, `select_font`, `select_font_ref`, `resolve_font`, `get_page_content`, `concat_content_streams`, `do_xobject`, `render_image`, `render_form_xobject`, `apply_extgstate`, `image_to_rgba`, `cs_from_name`, `render_text_string`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Image pixel layout](../invariant/image-pixel-layout.md)
- [Page content assembly](../invariant/page-content-assembly.md)
- [Font resolution](../invariant/font-resolution.md)
- [Content stream recursion](../invariant/content-stream-recursion.md)
- [resource name scope](../invariant/resource-scope.md)

## Blast radius
- [Render interpreter](render-interpreter.md) — the original. Operator behaviour fixed on the raster side does not propagate here.
- [Image decoding](image-decoding.md), [ToUnicode](tounicode.md), [Color spaces](color-spaces.md) — input.
- [Render API](render-api.md) — `render_page_to_svg`.
- [CLI](cli.md) — `render -F svg`, `convert` SVG output.

## Known holes / open
- The test checks only that the output contains `<svg`.
- `BMC`/`BDC`/`EMC`/`MP`/`DP` are no-ops and `Do` ignores `/OC`, so OFF optional content is emitted. Measured 2026-10-07: fills inside `/OC /Off BDC`, inside an `/On` section nested in it, and in a Form XObject with `/OC <Off>` all appear in the output. To reuse the visibility helper #44 adds in core. Tracked: #218
- Annotations are not drawn at all: neither `render_page_to_svg` nor `svg_device.rs` reads `/Annots` (inferred).
