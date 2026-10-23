# Render clipping

## What it is
Applies `W`/`W*` clipping paths to the device's clip mask and restores them on q/Q.

## Governing decisions
**None.**

## Design model
- The clip **lives on the device**, not in the graphics state. `GraphicsState` holds only `has_clip: bool`.
- **A Form XObject drawn by `Do` is clipped to its `/BBox`** (ISO 32000-1 §8.10.1 step c, read raw; the 32000-2 clause was not read, #365). `do_xobject` pushes the state, saves `device.clip_mask`, calls `clip_to_bbox` (BBox through `/Matrix` × CTM × page transform, intersected with any clip set), renders, then restores the clip mask and pops. The clip mask is restored by hand because popping the state does not restore it (#50). Measured by mutation: dropping the restore fails `content_after_the_do_is_not_clipped_to_the_bbox` and `the_page_clip_before_the_do_is_the_one_after_it`.
  - A transparency-group form renders into its own pixmap without the clip and is composited through `draw_pixmap` with the restored clip, so it is clipped too. Measured by mutation: setting the clip inside `render_form_xobject` before its group branch, instead of in `do_xobject`, keeps every test green.
  - Until #365 a form painted outside its `/BBox` onto the page (measured: a full-page fill in a `[100 100 200 200]` form painted 230,000 pixels outside the box on a 400 × 600 page).
- `Q` clears the device clip only when the restored state has no clip ("simplified: just clear for now"). If the parent also had a clip, the inner clip survives past `Q` (inferred).

## Code
- `justpdf-render/src/interpreter.rs` — `apply_clip`, `fill_current_path`, `stroke_current_path`, `clip_to_bbox`, `do_xobject`
- `justpdf-render/tests/render_form_bbox.rs` — the form `/BBox` clip: plain, `/Matrix` and CTM, transparency group, nested, and the clip after the `Do`
- `justpdf-render/src/device.rs` — `set_clip_path`, `intersect_clip_path`, `clear_clip`

## Reference behaviour
**None.** Clauses to compare against: ISO 32000-2 §8.5.4, and §8.10.1 for the form `/BBox` clip.

## Cross-cutting invariants
**None.**

## Blast radius
- [Raster device](raster-device.md) — where the clip is kept.
- [Render transparency](render-transparency.md) — restoring the clip after applying a soft mask (`restore_clip_after_soft_mask`) is empty.
- [Render interpreter](render-interpreter.md) — the q/Q stack.

## Known holes / open
- Nested clips are not restored on `Q` (above, inferred; no test).
- **Every clip is a full-page mask**, so the form `/BBox` clip costs a page-sized `Mask` per `Do` (measured 2026-10-23, release, best of 5, a 400 × 600 pt page of `n` 8 × 8 forms: 72 dpi, 2000 forms 14.5 → 39.2 ms; 300 dpi, 100 forms 4.4 → 27.9 ms, 2000 forms 99.7 → 700 ms). 112 real pages (21 local files, one with forms: `testpdf.pdf`, 2880 `Do`) rendered pixel-identical before and after; their timing was too noisy to read. MuPDF keeps a rectangular clip as a scissor rectangle with no mask, and sizes a non-rectangular clip's mask to its bounds (`fz_draw_clip_path`, `source/fitz/draw-device.c`, master, read raw). The maintainer chose to land the clip with this cost and take the device scissor separately (judgement, #365, 2026-10-23, shown these numbers and the MuPDF fast path).
- Tracked: #50 (clip restore)
