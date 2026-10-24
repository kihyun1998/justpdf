# Render transparency (groups, soft masks, blending)

## What it is
Applies the alpha, blend mode and soft mask from an ExtGState (`gs`), and draws a transparency group Form XObject into a temporary pixmap and composites it.

## Governing decisions
**None.**

## Design model
- Soft masks are Luminosity and Alpha only. Any other subtype returns early.
- The mask is built by drawing the `/G` form the moment `gs` is met (MuPDF defers it until painting). If `/G` is already running, applying that `/SMask` is skipped altogether — [Content stream recursion](../invariant/content-stream-recursion.md).
- The `/G` form is drawn in its own `/Resources` scope, else the scope at the `gs` — [resource name scope](../invariant/resource-scope.md).
- `/BC` (backdrop color) is ignored and black is used.
- **The soft mask is a graphics-state parameter apart from the clip** (ISO 32000-1 Table 58, `SMask`: setting it replaces the old value rather than intersecting, unlike the clip; read raw, #50). `SoftMask::mask` is a `SharedMask`, so `q` shares it. Fills, strokes and image XObjects draw through `draw_mask` (clip × soft mask, computed per draw and never stored), so the mask ends with its `Q` or `/SMask /None`, and each draw under it is masked once. #129 is to call the same function from the paths that skip the soft mask.
  - Until #50 each masked draw multiplied the soft mask into the device clip and never took it out (measured: the mask applied after its `Q` to the end of the page, and two fills under a uniform 0.5 mask came out (128,128,255) and (192,192,255)).
  - Glyph fills, pattern fills and strokes, `sh`, stencil masks (`/ImageMask`, `render_image_mask`) and the transparency-group composite use the clip only (#129).
  - A draw under both a clip and a soft mask allocates their product, a page-sized mask, per draw, as before #50.
  - Measured 2026-10-24, release, a 400 × 600 pt page of 1000 small fills under a uniform luminosity mask, master → #50: 300 dpi, fills alone 717.5 → 43.9 ms; each fill in its own `q … Q` 832.7 → 41.0 ms (best of 5). Before #50 every masked draw copied and multiplied a page mask, and every `q` copied the soft mask. With a clip as well, unchanged within noise (best of 15, two alternating rounds: master 461 / 406 ms, #50 445 / 435 ms). 112 real pages (21 local files) rendered pixel-identical before and after.
- Luminosity weights are Rec.709. [Compress grayscale](compress-grayscale.md) uses Rec.601.
- The ExtGState's `/Font` is ignored (text extraction does the same).

## Code
- `justpdf-render/src/interpreter.rs` — `render_form_xobject`, `render_transparency_group`, `render_form_xobject_direct`, `apply_extgstate`, `apply_soft_mask`, `render_soft_mask`, `pixmap_to_mask`, `draw_mask`
- `justpdf-render/src/graphics_state.rs` — `SoftMask`, `PdfBlendMode`

## Reference behaviour
**None.** Clause to compare against: ISO 32000-2 §11.

## Cross-cutting invariants
- [Content stream recursion](../invariant/content-stream-recursion.md)
- [resource name scope](../invariant/resource-scope.md)

## Blast radius
- [Render clipping](render-clipping.md) — `draw_mask` combines the clip with the soft mask.
- [Raster device](raster-device.md) — draws through the combined mask.
- [Render images](render-images.md) — SMask.

## Known holes / open
- A form whose `/Group` is an indirect reference is not treated as a transparency group — only a direct dictionary is — so its content is drawn straight onto the page with the inner blend mode (measured 2026-09-30: a Multiply-composited group paints over what it should multiply with). Tracked: #129
- The soft mask `/G` and transparency group forms inherit the caller's `ca`/`CA`, blend and soft mask. Tracked: #128
- An indirect-reference `/SMask` is ignored, and the mask is not applied to pattern fills. Tracked: #129
