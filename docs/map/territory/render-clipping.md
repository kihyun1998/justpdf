# Render clipping

## What it is
Applies `W`/`W*` clipping paths to the device's clip mask and restores them on q/Q.

## Governing decisions
**None.**

## Design model
- The clip **lives on the device**, not in the graphics state. `GraphicsState` holds only `has_clip: bool`.
- `Q` clears the device clip only when the restored state has no clip ("simplified: just clear for now"). If the parent also had a clip, the inner clip survives past `Q` (inferred).

## Code
- `justpdf-render/src/interpreter.rs` — `apply_clip`, `fill_current_path`, `stroke_current_path`
- `justpdf-render/src/device.rs` — `set_clip_path`, `intersect_clip_path`, `clear_clip`

## Reference behaviour
**None.** Clause to compare against: ISO 32000-2 §8.5.4.

## Cross-cutting invariants
**None.**

## Blast radius
- [Raster device](raster-device.md) — where the clip is kept.
- [Render transparency](render-transparency.md) — restoring the clip after applying a soft mask (`restore_clip_after_soft_mask`) is empty.
- [Render interpreter](render-interpreter.md) — the q/Q stack.

## Known holes / open
- Nested clips are not restored on `Q` (above, inferred; no test).
- Tracked: #50 (clip restore)
