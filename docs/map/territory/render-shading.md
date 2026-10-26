# Render shading

## What it is
Rasterizes type 1–7 shadings (function-based, axial, radial, mesh) for the `sh` operator and shading patterns.

## Governing decisions
**None.**

## Design model
- **Function-based shading gets its function from the interpreter** (#47): `render_shading` takes `function: Option<&PdfFunction>`, read with `PdfFunction::resolve`, so Type 0 and filtered Type 4 functions paint their colours. Until #47 `shading.rs` parsed `/Function` itself without decoding: a sampled function fell back to gray (127,127,127) and a Flate Type 4 came out garbled (129,125,0) where red was right (measured 2026-10-25).
- **Every shading type evaluates `/Function` through core `PdfFunction`** (#233), read once by the interpreter with `PdfFunction::resolve` (a single function, or an array of 1-out functions concatenated) and passed to `render_shading`. Axial and radial sample it at 64 points across `/Domain` (default `[0 1]`) into gradient stops (`function_stops`); without a function they fall back to black → white. Meshes (4–7) with a function read one component `t` per vertex in the last `/Decode` pair; `mesh_colours` puts t's index (0–255) in the first colour channel and builds a 256-entry table from the function, and the rasterizer interpolates the index and looks the colour up, so `t` is interpolated before the function is applied (ISO 32000-2 §8.7.4.5.5). Without a function a vertex's colour is its components in the colour space, as before.
  - Until #233 (measured 2026-10-07): an array of three 1-out functions drew black instead of green, a Type 2 `/N 2` ramp was linear (129 where 64 is right), a stitching function with referenced sub-functions fell back to a gray ramp, `/Encode [1 0]` was ignored, and a mesh with `/Function` drew nothing. Pinned by `tests/render_shading_functions.rs`; mutations dropping arrays, the function sampling, the stop count, the one-component read or the `t` table each fail a named test.
- **Free-form (type 4) edge flags follow ISO 32000-1 §8.7.4.5.5** (read raw): a flag-0 vertex starts a triangle and the next two vertices complete it whatever their flags; then flag 1 continues from side (vb, vc) and flag 2 from (va, vc). Until #233 every flag-0 vertex cleared the pending vertices, so a mesh written the usual way (each triangle's vertices flagged 0) drew nothing at all (measured 2026-10-25: a blue triangle drew white). Found while working #233; fixed there because its tests could not draw a type-4 mesh otherwise.
  - The rasterizer still truncates interpolated colours (an RGB 255 can come out 254).
- Patch meshes ignore the Bézier control points ("approximation").
- **Axial and radial shadings paint only the region `/Extend` allows** (#384, ISO 32000-2 §8.7.4.5.3–4). tiny-skia 0.11 has no spread mode that leaves the outside transparent (`Pad`, `Reflect`, `Repeat` only), so the gradient keeps `SpreadMode::Pad` and the *filled path* is the region: axial fills the band between the lines through its ends perpendicular to the axis, reaching `FAR` past an end it extends (`axial_region`); radial without `Extend[1]` fills the convex hull of the start and end disks (`disks_hull`, a polygon through 128 points per circle), and with it the `FAR` square. The region is the shape, so it composes with the clip mask the fill already takes. Until #384 both filled a fixed −10000…20000 square, so `/Extend [false false]` (the default) painted the end colours across the whole clip (measured 2026-10-26: an axial 25→75 shading painted red at x = 10 and blue at x = 90). Pinned by `tests/render_shading_extend.rs`; mutations dropping or swapping the flags, filling the square for radial, dropping the start disk or the upper hull chain, or using an axis-aligned strip for the band each fail a named test.
  - Radial `Extend[0]` paints nothing here: tiny-skia's `RadialGradient` is a two-point conical gradient with the start radius fixed at 0, so `r0` is ignored and the circles for s < 0 have no radius (inferred, read from `tiny-skia-0.11.4/src/shaders/radial_gradient.rs`). The hull already uses the real `r0`.
- **Colours convert through the shading's resolved `/ColorSpace`** (#231): `render_shading` takes a `&ColorSpace` that the interpreter resolves with `ColorSpace::resolve` (name, array or reference; DeviceRGB without one), for `sh` and shading patterns alike. `components_to_color` converts through `Color::to_rgb` and truncates each channel as before, so device spaces are unchanged (the DeviceGray control keeps its pixels), and a mesh vertex carries `num_components` values. Until #231 `/ColorSpace` was read with `as_name` only and anything but Gray/CMYK was taken as RGB (measured 2026-10-06: a Separation and an ICCBased gray shading both came out red, (27,0,0) → (231,0,0)). Pinned by `tests/render_colour_spaces.rs`.
- An unknown type is ignored as "unsupported".

## Code
- `justpdf-render/src/shading.rs` — `render_shading`, `extend`, `axial_region`, `disks_hull`, `convex_hull`, `parse_gouraud_triangles`, `parse_lattice_vertices`, `parse_patch_mesh`, `rasterize_triangle`, `function_stops`, `mesh_colours`, `components_to_color`, `color_space_components`
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
- Radial shadings ignore the start radius `r0` (tiny-skia `RadialGradient` takes one radius): a shading with `r0 > 0` draws its colours from a point instead of from the start circle (inferred, not rendered). Tracked: #389
- Type 7 (tensor-product) patches take their corners from stream points 0, 3, 12, 15; pdf.js `_decodeType7Shading` maps stream points 0, 3, 6, 9 to the corners and 12–15 to the interior control points, as for Type 6 (inferred: read against pdf.js `src/core/pattern.js`, not reproduced by rendering). The branch that picks `c1` is identical for both types, which clippy flags as `if_same_then_else`; it is allowed at the site rather than merged so the selection stays visible. Tracked: #196
