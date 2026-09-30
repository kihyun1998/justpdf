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
- The body of `restore_clip_after_soft_mask` is empty ("we'll just leave the combined mask in place").
- Luminosity weights are Rec.709. [compress-grayscale](compress-grayscale.md) uses Rec.601.
- The ExtGState's `/Font` is ignored (text extraction does the same).

## Code
- `justpdf-render/src/interpreter.rs` — `render_form_xobject`, `render_transparency_group`, `render_form_xobject_direct`, `apply_extgstate`, `apply_soft_mask`, `render_soft_mask`, `pixmap_to_mask`, `apply_soft_mask_to_device`, `restore_clip_after_soft_mask`
- `justpdf-render/src/graphics_state.rs` — `SoftMask`, `PdfBlendMode`

## Reference behaviour
**None.** Clause to compare against: ISO 32000-2 §11.

## Cross-cutting invariants
- [Content stream recursion](../invariant/content-stream-recursion.md)
- [resource name scope](../invariant/resource-scope.md)

## Blast radius
- [Render clipping](render-clipping.md) — the mask and the clip use the same device field.
- [Raster device](raster-device.md) — edits `clip_mask` directly.
- [Render images](render-images.md) — SMask.

## Known holes / open
- A form whose `/Group` is an indirect reference is not treated as a transparency group — only a direct dictionary is — so its content is drawn straight onto the page with the inner blend mode (measured 2026-09-30: a Multiply-composited group paints over what it should multiply with). Tracked: #129
- Restoring the clip after a soft mask is not implemented (above).
- The soft mask `/G` and transparency group forms inherit the caller's `ca`/`CA`, blend and soft mask. Tracked: #128
- An indirect-reference `/SMask` is ignored, and the mask is not applied to pattern fills. Tracked: #129
- Tracked: #50 (clip restore)
