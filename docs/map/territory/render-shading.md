# Render shading

## What it is
Rasterizes type 1–7 shadings (function-based, axial, radial, mesh) for the `sh` operator and shading patterns.

## Governing decisions
**None.**

## Design model
- Only function-based (type 1) uses core `PdfFunction`. Axial and radial read `C0`/`C1`/`Bounds` again by hand (type 2 and 3 functions only) — separate from what [PDF functions](pdf-functions.md) supports.
- Patch meshes ignore the Bézier control points ("approximation").
- Color conversion is a name-only copy (`components_to_color`, `color_space_components`) — one of the color conversion copies in [Image pixel layout](../invariant/image-pixel-layout.md).
- An unknown type is ignored as "unsupported".

## Code
- `justpdf-render/src/shading.rs` — `render_shading`, `parse_gouraud_triangles`, `parse_lattice_vertices`, `parse_patch_mesh`, `rasterize_triangle`, `extract_stops_from_function`, `extract_colors_from_function`, `components_to_color`, `color_space_components`
- `justpdf-render/src/interpreter.rs` — `render_shading`, `render_shading_pattern`

## Reference behaviour
**None.** Clause to compare against: ISO 32000-2 §8.7.4.5.

## Cross-cutting invariants
- [Image pixel layout](../invariant/image-pixel-layout.md) — the color conversion copy.

## Blast radius
- [PDF functions](pdf-functions.md) — the function parser (inferred to fail on Type 4 because the stream is not decoded).
- [Color spaces](color-spaces.md) — bypassed.
- [Render tiling patterns](render-tiling-patterns.md) — shares pattern resolution.
- [SVG renderer](svg-renderer.md) — skips shadings.

## Known holes / open
- Type 7 (tensor-product) patches take their corners from stream points 0, 3, 12, 15; pdf.js `_decodeType7Shading` maps stream points 0, 3, 6, 9 to the corners and 12–15 to the interior control points, as for Type 6 (inferred: read against pdf.js `src/core/pattern.js`, not reproduced by rendering). The branch that picks `c1` is identical for both types, which clippy flags as `if_same_then_else`; it is allowed at the site rather than merged so the selection stays visible.
- Tracked: #47 (function Type 0 and Type 4)
