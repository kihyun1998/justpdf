# Render transparency (groups, soft masks, blending)

## What it is
Applies the alpha, blend mode and soft mask from an ExtGState (`gs`), and draws a transparency group Form XObject into a temporary pixmap and composites it.

## Governing decisions
**None.**

## Design model
- Soft masks are Luminosity and Alpha only. Any other subtype returns early.
- The mask is built by drawing the `/G` form the moment `gs` is met (MuPDF defers it until painting). If `/G` is already running, applying that `/SMask` is skipped altogether — [Content stream recursion](../invariant/content-stream-recursion.md).
- The `/G` form is drawn in its own `/Resources` scope, else the scope at the `gs` — [resource name scope](../invariant/resource-scope.md).
- **A Luminosity mask's group is composited over `/BC`**, given in the group's `/CS`, black without one; an Alpha mask ignores it (ISO 32000-1 Table 144, read raw, #374). The mask pixmap is filled with the backdrop before `/G` runs, so it shows where `/G` paints nothing, including outside `/G`'s `/BBox` (MuPDF master `begin_softmask` uses an unbounded area for luminosity masks, read raw). `mask_backdrop` resolves the group's `/CS` with `ColorSpace::resolve` (since #46, so an ICCBased space converts through its `/Alternate` or `/N`) and converts with `Color::to_rgb`. Without a `/CS`, or when its component count differs from `/BC`'s, the space is taken from the count (1, 3, 4: gray, RGB, CMYK). Until #46 an ICCBased `/CS` always went through that count fallback, because `ColorSpace::from_pdf_object` never read the stream.
- **`/TR` maps the mask values**, for both subtypes, clamped to 0..1 (Table 144). `transfer_table` reads it with `PdfFunction::resolve` (#47) into a 256-entry table applied after `pixmap_to_mask`; `Identity`, no `/TR`, and a function that does not parse leave the values as they are. Until #47 a filtered Type 4 `/TR` was parsed undecoded and acted as `Identity`.
  - Until #374 both were ignored (measured 2026-10-24 on master, a red full-page fill, centre pixel): `/BC [1]` with an empty `/G` gave white instead of red, and an inverting `/TR` over a white `/G` gave red instead of white. Pinned by `tests/render_soft_mask_dict.rs`.
- **The soft mask is a graphics-state parameter apart from the clip** (ISO 32000-1 Table 58, `SMask`: setting it replaces the old value rather than intersecting, unlike the clip; read raw, #50). `SoftMask::mask` is a `SharedMask`, so `q` shares it. Every painting path draws through `draw_mask` (clip × soft mask, never stored in the graphics state), so the mask ends with its `Q` or `/SMask /None`, and each draw under it is masked once. The paths: solid fills and strokes, pattern fills and strokes, `sh`, glyph fills (outlines and the placeholder rectangle), stencil masks (`render_image_mask`), and image XObjects (#129).
  - Until #50 each masked draw multiplied the soft mask into the device clip and never took it out (measured: the mask applied after its `Q` to the end of the page, and two fills under a uniform 0.5 mask came out (128,128,255) and (192,192,255)).
  - **An image XObject with its own `/SMask` or `/Mask` is drawn with the clip only**: "Either form of mask in the image dictionary shall override the current soft mask in the graphics state" (ISO 32000-1 §11.6.4.3, read raw, #129). Until #129 such an image was masked twice (measured: an opaque red image with an all-255 `/SMask`, under a half-white soft mask, drew white on the masked-out half).
  - A shading pattern's pixmap is rendered with the clip only, and the soft mask applies when the pattern fill is drawn, so it is applied once. Measured by mutation: rendering the pixmap through `draw_mask` as well fails `a_shading_pattern_fill_is_masked_once`.
- **A soft mask `/G` form and a transparency group's content start from blend Normal, alpha 1 and no soft mask** (`reset_transparency`): "the current blend mode in the graphics state shall be initialized to Normal, the current stroking and nonstroking alpha constants to 1.0, and the current soft mask to None", so that they are "not applied twice" (ISO 32000-1 §11.6.6 and its NOTE 1, read raw, #128). The group is then composited once, through `draw_mask` (the clip and soft mask current at `Do`) with the `ca` and blend mode current at `Do`. MuPDF master does the same, read raw: `begin_softmask` clears the soft mask and sets alpha 1 before the mask form, and `pdf_run_xobject` passes the `Do`-time blend and alpha to `fz_begin_group` and resets them inside.
  - Until #128 the mask and group forms inherited them (measured 2026-10-24 on master, a red fill at the page centre): `ca 0.5` with a full-white soft mask gave (255,191,191), where (255,128,128) is right; `ca 0.5` and a group form gave (255,192,192), alpha applied twice; a second soft mask set while the first was current was drawn through the first, so the right half stayed white. The group composite used the clip only; a group under a half-white soft mask happened to come out right, because the mask was applied once inside the group instead.
  - Until #129 pattern fills and strokes, `sh`, text and stencil masks ignored the soft mask (measured 2026-10-06 on master: each painted the masked-out half red). Pinned by `tests/render_soft_mask_paths.rs`, one test per path; a mutation reverting either glyph site, the pattern fill or the image rule fails its test.
- **`/SMask` in an ExtGState and `/Group` on a form are read through indirect references** (`dict_entry`), the same as the direct dictionary, including the group's `/I` (#129). The mask's `/G` already followed a reference. Until #129 both were read only as direct dictionaries (measured: `/SMask 6 0 R` left the fill unmasked, and a non-isolated Multiply group written as `/Group 6 0 R` painted (229,229,204) over black where the direct dictionary gives (0,0,0); in a real magazine file, a gutter shadow drew as a solid band and a title was painted over).
  - **The clip × soft mask product is kept** in `RenderInterpreter::combined_mask` with the clip and soft mask it was made from, and reused while both are the same masks (`SharedMask::ptr_eq`). Holding the two keeps them alive, so a freed mask's address cannot be reused and mistaken for one of them. The cache also holds a reference to the clip, so the next `W` copies the clip once rather than changing it in place. Measured 2026-10-24, release, 1300 Helvetica glyphs at 10 pt on a 400 × 600 pt page under a clip and a uniform mask, 300 dpi, best of 9: master 106.1 ms; #129 without the cache 794.9 ms (each glyph made its own page-sized product); with it 45.0 ms. Measured by mutation: dropping the clip or the soft-mask comparison fails `a_new_clip_under_the_same_soft_mask_is_used` or `a_new_soft_mask_under_the_same_clip_is_used`.
  - Measured 2026-10-24, release, a 400 × 600 pt page of 1000 small fills under a uniform luminosity mask, master → #50: 300 dpi, fills alone 717.5 → 43.9 ms; each fill in its own `q … Q` 832.7 → 41.0 ms (best of 5). Before #50 every masked draw copied and multiplied a page mask, and every `q` copied the soft mask. With a clip as well, unchanged within noise (best of 15, two alternating rounds: master 461 / 406 ms, #50 445 / 435 ms). 112 real pages (21 local files) rendered pixel-identical before and after.
- Luminosity weights are Rec.709. [Compress grayscale](compress-grayscale.md) uses Rec.601.
- The ExtGState's `/Font` is ignored (text extraction does the same).

## Code
- `justpdf-render/src/interpreter.rs` — `render_form_xobject`, `render_transparency_group`, `render_form_xobject_direct`, `apply_extgstate`, `apply_soft_mask`, `render_soft_mask`, `pixmap_to_mask`, `draw_mask`, `combined_mask`, `dict_entry`, `nested_stream`, `reset_transparency`, `mask_backdrop`, `transfer_table`
- `justpdf-render/tests/render_soft_mask_dict.rs` — `/BC` and `/TR`
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
- The group's `/K` (knockout) and `/CS` (blending colour space) are not read (measured for `/K`, 2026-10-24). Tracked: #375
