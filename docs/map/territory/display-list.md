# Display list and tile rendering

## What it is
A structure that records drawing commands and replays them onto a pixmap, replays them under an extra transform, drops no-op command pairs, or renders them tile by tile. Nothing in the product records into it.

## Governing decisions
**None.** [ADR-0006](../../adr/0006-breaking-changes-are-allowed-before-1-0.md) and [the out-of-scope record on narrowing the render API](../../../.out-of-scope/narrowing-render-public-api.md) (2026-10-24, #371) keep `display_list` public although unused: narrowing it would only make the compiler flag `DisplayList` as dead code. They do not decide whether it should be connected.

## Design model
Read from the source; no record states it.

- **The interpreter does not record into it.** Its only use outside the file is `pub mod display_list` in `lib.rs` (re-verified: `git grep DisplayList` outside `justpdf-render/src` finds only `.out-of-scope/`).
- **`DisplayCommand` holds fills, strokes, images, clip push/pop, save/restore and transparency groups** with opacity, blend mode and isolation. It has no command for a soft mask, a shading drawn per pixel, a pattern shader or a glyph, so it cannot hold what [the interpreter](render-interpreter.md) draws today (inferred, `DisplayCommand`).
- Replay keeps clips on its own stack: `PushClip` intersects the new mask with the current one and `PopClip` restores it; `Save` and `Restore` do nothing during replay (re-verified, `replay_with_transform`).
- `optimize` removes adjacent `Save`/`Restore`, `BeginGroup`/`EndGroup` and `PushClip`/`PopClip` pairs until none is left (re-verified; its doc comment names only the first two).
- `render_tiled` renders each tile with `render_tile` and copies it into an output pixmap of the full size, so it does not lower peak memory below one full page, whatever its doc comment says (inferred).

## Code
- `justpdf-render/src/display_list.rs` — `DisplayList`, `DisplayCommand`, `DisplayList::push`, `replay`, `replay_with_transform`, `bounds`, `optimize`, `render_tile`, `render_tiled`, `intersect_masks`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Render interpreter](render-interpreter.md) — the side that would record into it; anything the interpreter draws that `DisplayCommand` cannot express (soft masks, per-pixel shadings, pattern shaders) would have to be added here first.
- [Render API](render-api.md) — `parallel` is per-page parallelism, not per-tile; tiling a page would go through here.

## Known holes / open
- Tile rendering (`DisplayList::render_tile`, `render_tiled`) is not connected to the product path.
- `render_tiled` divides by its `tile_size`, so 0 panics (inferred). Tracked: #454
- Whether the display list is meant to be connected or removed is not recorded; the out-of-scope record defers narrowing to before 1.0 (open question for the maintainer).
