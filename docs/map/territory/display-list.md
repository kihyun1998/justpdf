# Display list and tile rendering

## What it is
A structure that records drawing commands and then replays them, replays them under a transform, optimizes them, or renders them tile by tile.

## Governing decisions
**None.**

## Design model
- The interpreter does not record into it. Its only use outside the file is `pub mod display_list` in `lib.rs`.

## Code
- `justpdf-render/src/display_list.rs` — `DisplayList`, `DisplayCommand`, `replay`, `replay_with_transform`, `optimize`, `render_tile`, `render_tiled`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Render interpreter](render-interpreter.md) — the side that should connect to it.
- [Render API](render-api.md) — `parallel` is per-page parallelism, not per-tile.

## Known holes / open
- Tile rendering (`DisplayList::render_tile`) is not connected to the product path.
