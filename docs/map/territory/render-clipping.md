# Render clipping

## What it is
Keeps the current clipping path in the graphics state, intersects `W`/`W*` paths into it, and restores it on `Q`.

## Governing decisions
**None.**

## Design model
- **The clip lives in the graphics state** (ISO 32000-1 §8.5.4 and its NOTE 2: `Q` reverts the clip to the value saved by `q`; read raw, #50). `GraphicsState::clip` is an `Option<SharedMask>`, `None` meaning no clip. `SharedMask` wraps an `Arc<Mask>`, so `q` (cloning the state) shares the page-sized mask rather than copying it. `W`/`W*` and the form `/BBox` clip go through `intersect_clip`, which copies the mask only while another saved state shares it (`Arc::make_mut`). `Q` pops the state and nothing else.
  - The device holds no clip. Each drawing method takes the mask to draw through as an argument.
  - Until #50 the clip was one mask on the device, and `Q` cleared it only when the restored state had no clip. A nested clip survived `Q` (measured: `q 0 0 100 200 re W n q 0 0 50 200 re W n Q` then a full fill left x = 75 white).
- **The soft mask is not part of the clip.** `draw_mask` gives the mask one draw is painted through: the clip, the soft mask, or their product. It is computed per draw and never stored ([Render transparency](render-transparency.md)).
- **Content drawn into another pixmap starts unclipped:** the pattern tile (a pixmap of another size, so a page mask cannot apply), the soft mask `/G` and the transparency group set `clip = None` after their push. The group is clipped when `draw_pixmap` composites it. Measured by mutation: keeping the page clip in the tile fails `a_tiling_pattern_fill_is_clipped`. Keeping it in the group or in `/G` changes no pixel, since a clip can only shrink until the `Q` that also ends the group or the soft mask (equivalent mutation, inferred).
- **A Form XObject drawn by `Do` is clipped to its `/BBox`** (ISO 32000-1 §8.10.1 step c, read raw; the 32000-2 clause was not read, #365). `do_xobject` pushes the state, calls `clip_to_bbox` (BBox through `/Matrix` × CTM × page transform, intersected with any clip set), renders, then pops, which restores the clip. Until #50 the device clip mask was saved and restored by hand around the form (measured by mutation then: dropping that restore failed `content_after_the_do_is_not_clipped_to_the_bbox` and `the_page_clip_before_the_do_is_the_one_after_it`).
  - A transparency-group form renders into its own pixmap without the clip and is composited through `draw_pixmap` with the restored clip, so it is clipped too. Measured by mutation: setting the clip inside `render_form_xobject` before its group branch, instead of in `do_xobject`, keeps every test green.
  - Until #365 a form painted outside its `/BBox` onto the page (measured: a full-page fill in a `[100 100 200 200]` form painted 230,000 pixels outside the box on a 400 × 600 page).

## Code
- `justpdf-render/src/interpreter.rs` — `apply_clip`, `intersect_clip`, `draw_mask`, `clip_to_bbox`, `do_xobject`, `fill_current_path`, `stroke_current_path`
- `justpdf-render/src/graphics_state.rs` — `GraphicsState::clip`, `SharedMask`
- `justpdf-render/tests/render_clip_state.rs` — `Q` restoring a nested clip, the soft mask apart from the clip, and the clip under patterns and `sh`
- `justpdf-render/tests/render_form_bbox.rs` — the form `/BBox` clip: plain, `/Matrix` and CTM, transparency group, nested, and the clip after the `Do`
- `justpdf-render/src/device.rs` — `clip_mask` (a fresh mask for a path); every drawing method takes its clip

## Reference behaviour
ISO 32000-1 §8.5.4 (read raw for #50); §8.10.1 for the form `/BBox` clip. The ISO 32000-2 clauses were not read.

## Cross-cutting invariants
**None.**

## Blast radius
- [Raster device](raster-device.md) — draws through the mask it is given.
- [Render transparency](render-transparency.md) — `draw_mask` combines the soft mask with the clip per draw.
- [Render interpreter](render-interpreter.md) — the q/Q stack.

## Known holes / open
- **Every clip is a full-page mask**, so the form `/BBox` clip costs a page-sized `Mask` per `Do` (measured 2026-10-23, release, best of 5, a 400 × 600 pt page of `n` 8 × 8 forms: 72 dpi, 2000 forms 14.5 → 39.2 ms; 300 dpi, 100 forms 4.4 → 27.9 ms, 2000 forms 99.7 → 700 ms). 112 real pages (21 local files, one with forms: `testpdf.pdf`, 2880 `Do`) rendered pixel-identical before and after; their timing was too noisy to read. MuPDF keeps a rectangular clip as a scissor rectangle with no mask, and sizes a non-rectangular clip's mask to its bounds (`fz_draw_clip_path`, `source/fitz/draw-device.c`, master, read raw). The maintainer chose to land the clip with this cost and take the device scissor separately (judgement, #365, 2026-10-23, shown these numbers and the MuPDF fast path).
- Tracked: #369 (rectangular scissor)
