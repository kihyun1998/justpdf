# Render tiling patterns

## What it is
Resolves the pattern set by `scn`/`SCN`: a tiling pattern draws the tile and repeats it, and a shading pattern is handed to shading.

## Governing decisions
**None.**

## Design model
- `scn`/`SCN` resolves the pattern in the current scope and puts the object (`PatternSelection`) in the graphics state. The tile is drawn in the pattern's `/Resources` scope — [resource name scope](../invariant/resource-scope.md).
- Only path fills and strokes use the pattern. Text is not filled with the pattern (inferred).
- The tile is drawn while recorded as a running stream, and painting the same pattern again inside the tile paints with the fill color, without the pattern. The tile is drawn without the page's soft mask — [Content stream recursion](../invariant/content-stream-recursion.md). The tile runs through `nested_stream` (#128): an unmatched `q` in it stays in it, and the page's current path does not reach it. The pattern fill or stroke that paints with the tile goes through `draw_mask`, so the page's soft mask applies there (#129; `tile_is_drawn_without_the_page_soft_mask` asserts both halves).

- **The alpha constant applies once, at a different step for each pattern type** (`pattern_alpha`, #391). A shading pattern's pixmap is rendered opaque, so the fill or stroke that paints it takes `ca` or `CA`. A tiling cell runs with the page's graphics state and its own fills already carry the alpha, so the tile is painted at 1. Measured 2026-10-26: painting the tile at `ca` as well gave (255, 192, 192) for a red tile at `ca 0.5`, where a plain fill gives (255, 128, 128); `a_tiling_pattern_fill_paints_with_the_fill_alpha` fails under that mutation. Until #391 the shading pattern was painted at 1 too (`fill_path_with_pattern` and `stroke_path_with_pattern` hard-coded the opacity).
- **The cell is pattern space [0, XStep) × [0, YStep), placed through the pattern matrix once** (#396). `render_tiling_pattern` draws it with a scale only, at the resolution of `/Matrix` × CTM × page transform, and returns the shader transform from cell pixels back to pattern space and through `/Matrix`; tiny-skia composes the paint's transform (CTM × page) onto it, so the cell goes through each once. Until #396 the cell was drawn through `/Matrix` and at device scale, and painted with an identity shader transform, so it went through the page transform a second time (measured 2026-10-26 on master: a 20 pt cell of a 10 × 5 strip came out twice its size at 144 dpi and at 300 dpi, and a `/Matrix [2 0 0 2 0 0]` tiling had the strip at x = 25; at 72 dpi a cell that divides the page evenly happens to land right, because the page transform there is a flip about the page's centre). Pinned by `tests/render_tiling_placement.rs`; mutations painting with the identity, dropping the cell-to-pattern scale, leaving `/Matrix` out of the shader or drawing it into the cell as well, scaling after `/Matrix`, or taking the resolution without `/Matrix` each fail a named test (the last only `a_scaled_tiling_keeps_sharp_cell_edges`).
  - Pattern space follows the CTM current at the fill, as shading patterns do (#397).

## Code
- `justpdf-render/src/resources.rs` — `select_pattern`
- `justpdf-render/src/interpreter.rs` — `render_tiling_pattern`, `try_fill_with_pattern`, `try_stroke_with_pattern`, `render_pattern`, `pattern_alpha`
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
- The cell size is `(step * scale).ceil().max(1.0).min(2048.0)`: a NaN step gives a 1-pixel cell. `f64::clamp` would pass NaN through, cast it to 0 and make `Pixmap::new` fail, so clippy's `manual_clamp` is allowed there (inferred from the code).
- A tiling cell inherits the page's alpha and blend mode rather than starting from 1 and Normal and taking them when the tile is painted. The two agree for a cell of one opaque fill and differ where the cell's own objects overlap (inferred; ISO 32000-2 §8.7.3 and MuPDF `pdf_show_pattern` not yet read). Tracked: #393
- Shading patterns compose their `/Matrix` with the CTM current at the fill (`pattern_matrix.concat(&self.state.ctm)`), so a `cm` before the fill moves and scales the pattern like an `sh` (measured 2026-10-26: `0.5 0 0 0.5 0 0 cm` gave (150, 0, 104) at x = 20 where the fill without it gives (202, 0, 52)); the pattern matrix maps to the default space of the parent stream (ISO 32000-2 §8.7.3.1, inferred, not read raw). Tracked: #397
- The cell ignores `/BBox`: it always holds pattern space [0, XStep) × [0, YStep), so content left of or below the pattern origin is lost, and a `/BBox` larger than the step is cut to it (measured 2026-10-26: `/BBox [-5 -5 5 5]`, step 20, a red `-5 -5 10 10 re f` left (18, 18) white where (−2, −2) of the cell is red). Tracked: #399
