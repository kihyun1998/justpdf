# Render tiling patterns

## What it is
Resolves the pattern set by `scn`/`SCN`: a tiling pattern draws the tile and repeats it, and a shading pattern is handed to shading.

## Governing decisions
**None.**

## Design model
- `scn`/`SCN` resolves the pattern in the current scope and puts the object (`PatternSelection`) in the graphics state. The tile is drawn in the pattern's `/Resources` scope — [resource name scope](../invariant/resource-scope.md).
- Only path fills and strokes use the pattern. Text is not filled with the pattern (inferred).
- The tile is drawn while recorded as a running stream, and painting the same pattern again inside the tile paints with the fill color, without the pattern. The tile is drawn without the page's soft mask — [Content stream recursion](../invariant/content-stream-recursion.md).

## Code
- `justpdf-render/src/resources.rs` — `select_pattern`
- `justpdf-render/src/interpreter.rs` — `render_tiling_pattern`, `try_fill_with_pattern`, `try_stroke_with_pattern`, `render_pattern`
- `justpdf-render/src/device.rs` — `fill_path_with_pattern`, `stroke_path_with_pattern`

## Reference behaviour
**None.** Clause to compare against: ISO 32000-2 §8.7.3.

## Cross-cutting invariants
- [Content stream recursion](../invariant/content-stream-recursion.md)
- [resource name scope](../invariant/resource-scope.md)

## Blast radius
- [Render shading](render-shading.md) — shading patterns.
- [Raster device](raster-device.md) — pattern fills.
- [Glyph rendering](glyph-rendering.md) — pattern text is not supported.

## Known holes / open
- Filling or stroking with a pattern on the page (`try_fill_with_pattern`, `try_stroke_with_pattern`) does not apply the soft mask — only solid fills go through `apply_soft_mask_to_device` (observed 2026-09-29 while writing `tests/render_recursion.rs`). Tracked: #129
