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
- **The soft mask is a graphics-state parameter apart from the clip** (ISO 32000-1 Table 58, `SMask`: setting it replaces the old value rather than intersecting, unlike the clip; read raw, #50). `SoftMask::mask` is a `SharedMask`, so `q` shares it. Every painting path draws through `draw_mask` (clip × soft mask, never stored in the graphics state), so the mask ends with its `Q` or `/SMask /None`, and each draw under it is masked once. The paths: solid fills and strokes, pattern fills and strokes, `sh`, glyph fills (outlines and the placeholder rectangle), stencil masks (`render_image_mask`), and image XObjects (#129).
  - Until #50 each masked draw multiplied the soft mask into the device clip and never took it out (measured: the mask applied after its `Q` to the end of the page, and two fills under a uniform 0.5 mask came out (128,128,255) and (192,192,255)).
  - **An image XObject with its own `/SMask` or `/Mask` is drawn with the clip only**: "Either form of mask in the image dictionary shall override the current soft mask in the graphics state" (ISO 32000-1 §11.6.4.3, read raw, #129). Until #129 such an image was masked twice (measured: an opaque red image with an all-255 `/SMask`, under a half-white soft mask, drew white on the masked-out half).
  - A shading pattern's pixmap is rendered with the clip only, and the soft mask applies when the pattern fill is drawn, so it is applied once. Measured by mutation: rendering the pixmap through `draw_mask` as well fails `a_shading_pattern_fill_is_masked_once`.
  - The transparency-group composite still uses the clip only (#128).
  - Until #129 pattern fills and strokes, `sh`, text and stencil masks ignored the soft mask (measured 2026-10-06 on master: each painted the masked-out half red). Pinned by `tests/render_soft_mask_paths.rs`, one test per path; a mutation reverting either glyph site, the pattern fill or the image rule fails its test.
- **`/SMask` in an ExtGState and `/Group` on a form are read through indirect references** (`dict_entry`), the same as the direct dictionary, including the group's `/I` (#129). The mask's `/G` already followed a reference. Until #129 both were read only as direct dictionaries (measured: `/SMask 6 0 R` left the fill unmasked, and a non-isolated Multiply group written as `/Group 6 0 R` painted (229,229,204) over black where the direct dictionary gives (0,0,0); in a real magazine file, a gutter shadow drew as a solid band and a title was painted over).
  - **The clip × soft mask product is kept** in `RenderInterpreter::combined_mask` with the clip and soft mask it was made from, and reused while both are the same masks (`SharedMask::ptr_eq`). Holding the two keeps them alive, so a freed mask's address cannot be reused and mistaken for one of them. The cache also holds a reference to the clip, so the next `W` copies the clip once rather than changing it in place. Measured 2026-10-24, release, 1300 Helvetica glyphs at 10 pt on a 400 × 600 pt page under a clip and a uniform mask, 300 dpi, best of 9: master 106.1 ms; #129 without the cache 794.9 ms (each glyph made its own page-sized product); with it 45.0 ms. Measured by mutation: dropping the clip or the soft-mask comparison fails `a_new_clip_under_the_same_soft_mask_is_used` or `a_new_soft_mask_under_the_same_clip_is_used`.
  - Measured 2026-10-24, release, a 400 × 600 pt page of 1000 small fills under a uniform luminosity mask, master → #50: 300 dpi, fills alone 717.5 → 43.9 ms; each fill in its own `q … Q` 832.7 → 41.0 ms (best of 5). Before #50 every masked draw copied and multiplied a page mask, and every `q` copied the soft mask. With a clip as well, unchanged within noise (best of 15, two alternating rounds: master 461 / 406 ms, #50 445 / 435 ms). 112 real pages (21 local files) rendered pixel-identical before and after.
- Luminosity weights are Rec.709. [Compress grayscale](compress-grayscale.md) uses Rec.601.
- The ExtGState's `/Font` is ignored (text extraction does the same).

## Code
- `justpdf-render/src/interpreter.rs` — `render_form_xobject`, `render_transparency_group`, `render_form_xobject_direct`, `apply_extgstate`, `apply_soft_mask`, `render_soft_mask`, `pixmap_to_mask`, `draw_mask`, `combined_mask`, `dict_entry`
- `justpdf-render/src/graphics_state.rs` — `SoftMask`, `PdfBlendMode`
- `justpdf-render/tests/render_soft_mask_paths.rs` — the soft mask on every painting path, the image's own mask overriding it, and indirect `/SMask` and `/Group`

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
- The soft mask `/G` and transparency group forms inherit the caller's `ca`/`CA`, blend and soft mask. Tracked: #128
- `/BC` (backdrop colour; black is used), `/TR` (transfer function), and the group's `/K` and `/CS` are not read at all (inferred). Tracked: #374
