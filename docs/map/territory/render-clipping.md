# Render clipping

## What it is
Keeps the current clipping path in the graphics state, intersects `W`/`W*` paths into it, and restores it on `Q`.

## Governing decisions
**None.**

## Design model
Read from the source and the issues cited; no decision record states it.

- **The clip lives in the graphics state** (ISO 32000-1 §8.5.4 and its NOTE 2: `Q` reverts the clip to the value saved by `q`; read raw, #50). `GraphicsState::clip` is an `Option<SharedMask>`, `None` meaning no clip. `SharedMask` wraps an `Arc<Mask>`, so `q` (cloning the state) shares the page-sized mask rather than copying it. `W`/`W*` and the form `/BBox` clip go through `intersect_clip`, which copies the mask only while another saved state shares it (`Arc::make_mut`). `Q` pops the state and nothing else.
  - The device holds no clip. Each drawing method takes the mask to draw through as an argument.
  - Until #50 the clip was one mask on the device, and `Q` cleared it only when the restored state had no clip. A nested clip survived `Q` (measured: `q 0 0 100 200 re W n q 0 0 50 200 re W n Q` then a full fill left x = 75 white).
- **The soft mask is not part of the clip.** `draw_mask` gives the mask one draw is painted through: the clip, the soft mask, or their product. It is computed for a draw and never stored in the graphics state; the last product is cached ([Render transparency](render-transparency.md)).
- **`W`/`W*` take effect at the operator, not after the painting operator that ends the path** (`apply_clip` intersects the current path into the clip at once). So the painting operator of the same path is clipped by it: `0 0 1 RG 20 w 50 50 100 100 re W S` drew only the inner half of the stroke, pixel (45, 100) white and (55, 100) blue at 72 dpi (measured 2026-10-30 with the CLI at `88b2a08`). ISO 32000-2 §8.5.4 has the new clip apply after the path is painted (inferred: the clause was not read raw in this run). A fill (`re W f`) is unaffected, since its path lies inside itself. The SVG renderer does the same ([SVG renderer](svg-renderer.md)).
- **Content drawn into another pixmap starts unclipped:** the pattern tile (a pixmap of another size, so a page mask cannot apply), the soft mask `/G` and the transparency group set `clip = None` after their push. The group is clipped when `draw_pixmap` composites it. Measured by mutation: keeping the page clip in the tile fails `a_tiling_pattern_fill_is_clipped`. Keeping it in the group or in `/G` changes no pixel, since a clip can only shrink until the `Q` that also ends the group or the soft mask (equivalent mutation, inferred).
- **A Form XObject drawn by `Do` is clipped to its `/BBox`** (ISO 32000-1 §8.10.1 step c, read raw; the 32000-2 clause was not read, #365). `do_xobject` pushes the state, calls `clip_to_bbox` (BBox through `/Matrix` × CTM × page transform, intersected with any clip set), renders, then pops, which restores the clip. Until #50 the device clip mask was saved and restored by hand around the form (measured by mutation then, #50: dropping that restore failed `content_after_the_do_is_not_clipped_to_the_bbox` and `the_page_clip_before_the_do_is_the_one_after_it`).
  - A transparency-group form renders into its own pixmap without the clip and is composited through `draw_pixmap` with the restored clip, so it is clipped too. Measured by mutation: setting the clip inside `render_form_xobject` before its group branch, instead of in `do_xobject`, keeps every test green.
  - Until #365 a form painted outside its `/BBox` onto the page (measured, #365: a full-page fill in a `[100 100 200 200]` form painted 230,000 pixels outside the box on a 400 × 600 page).
- What bounds the input: a clip costs one page-sized byte mask, copied on the first `W` after each `q` that shares it; nothing bounds how many a page makes (inferred).

## Code
- `justpdf-render/src/interpreter.rs` — `apply_clip`, `intersect_clip`, `draw_mask`, `clip_to_bbox`, `do_xobject`, `fill_current_path`, `stroke_current_path`
- `justpdf-render/src/graphics_state.rs` — `GraphicsState::clip`, `SharedMask`, `SharedMask::make_mut`, `SharedMask::ptr_eq`
- `justpdf-render/tests/render_clip_state.rs` — `Q` restoring a nested clip, the soft mask apart from the clip, and the clip under patterns and `sh`
- `justpdf-render/tests/render_form_bbox.rs` — the form `/BBox` clip: plain, `/Matrix` and CTM, transparency group, nested, and the clip after the `Do`
- `justpdf-render/src/device.rs` — `clip_mask` (a fresh mask for a path); every drawing method takes its clip

## Reference behaviour
- ISO 32000-1 §8.5.4 and its NOTE 2 (read raw for #50) — `Q` restores the clip; §8.10.1 step c (read raw for #365) — the form `/BBox` clip. The ISO 32000-2 clauses were not read.
- MuPDF `source/fitz/draw-device.c` `fz_draw_clip_path`, master (read raw for #365) — a rectangular clip as a scissor with no mask, a non-rectangular one sized to its bounds.

## Cross-cutting invariants
- [Clip timing](../invariant/clip-timing.md)

## Blast radius
- [Raster device](raster-device.md) — every mask handed to it must be the pixmap's size; a change to how big a clip mask is (a scissor, a bounded mask, #369) changes what every drawing method receives.
- [Render shading](render-shading.md) — mesh triangles and radial pixels bypass tiny-skia and read the mask's coverage themselves (`rasterize_triangle`, `render_radial`), and `draw_area` scans it for bounds; both assume a page-sized byte mask.
- [Render transparency](render-transparency.md) — `draw_mask` multiplies the soft mask into the clip byte for byte, so the two must stay the same size and layout.
- [Render tiling patterns](render-tiling-patterns.md) — a tile starts with `clip = None` and clips each copy of its box with `intersect_clip` in cell pixels.
- [Render annotations](render-annotations.md) — an appearance is clipped to its `/BBox` through the same `clip_to_bbox`.
- [Render interpreter](render-interpreter.md) — the clip is restored by the `q`/`Q` stack and `nested_stream`; a clip kept anywhere else would escape `Q`.

## Known holes / open
- **Every clip is a full-page mask** (`clip_mask` sizes it to the pixmap; re-verified), so the form `/BBox` clip costs a page-sized `Mask` per `Do` (measured 2026-10-23, release, best of 5, a 400 × 600 pt page of `n` 8 × 8 forms: 72 dpi, 2000 forms 14.5 → 39.2 ms; 300 dpi, 100 forms 4.4 → 27.9 ms, 2000 forms 99.7 → 700 ms). 112 real pages (21 local files, one with forms: `testpdf.pdf`, 2880 `Do`) rendered pixel-identical before and after; their timing was too noisy to read. MuPDF keeps a rectangular clip as a scissor rectangle with no mask, and sizes a non-rectangular clip's mask to its bounds (`fz_draw_clip_path`, `source/fitz/draw-device.c`, master, read raw). The maintainer chose to land the clip with this cost and take the device scissor separately (#365, 2026-10-23, a judgement made after seeing these numbers and the MuPDF fast path). Tracked: #369
- A clip's bounds are not kept, so `draw_area` ([Render shading](render-shading.md), #394) finds them by scanning the page-sized mask once per shading paint. A device scissor (#369) would give them without the scan (inferred).
- `W` clips the painting operator that ends its own path, so `re W S` loses the outer half of its stroke (measured, above). Tracked: #418.
