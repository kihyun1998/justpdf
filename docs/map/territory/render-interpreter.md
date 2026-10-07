# Render interpreter (operator dispatch, graphics state)

## What it is
`RenderInterpreter` takes the page content and executes it operator by operator, keeping the graphics state (CTM, color, line, text state, blend) and sending drawing requests to the raster device. `interpreter.rs` is one file but holds several concepts — images, clipping, transparency, shading, patterns, glyphs, annotations and OCG each have their own note, and this note covers only the driver, the dispatch and the graphics state.

## Governing decisions
**None.** [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) decides only that rendering is a separate layer that pulls in `tiny-skia`.

## Design model
- **There is no device abstraction**: `RenderInterpreter` holds a `&mut PixmapDevice` directly (no `trait` defined). SVG output is not a device but a copy of the interpreter ([SVG renderer](svg-renderer.md)).
- `execute_op` is one big `match` on the operator string. An unknown operator is silently ignored by `_ => {}` (e.g. `ri`, `i`). Rendering Intent is not applied.
- `Tr` distinguishes only mode 3 (invisible) — there is no outline or clipping text (inferred).
- `d0`/`d1` are no-ops — [Type3 fonts](type3-fonts.md) are not supported.
- `v` (Bézier) is approximated without tracking the current point ("lossy without current point tracking").
- The color operators (`cs`/`CS`/`sc`/`scn`…) use the local `cs_from_name` (Device Gray/RGB/CMYK only, everything else is RGB) — the resource's color space, ICC, Indexed and Separation are not interpreted.
- Other content streams (Form XObject, tile, soft mask `/G`) are entered through `with_running_stream`: a stream met again while it is running is skipped — [Content stream recursion](../invariant/content-stream-recursion.md). Separately, a Form XObject is skipped past depth 10.
- The device color operators (`g`/`rg`/`k`, `G`/`RG`/`K`) clear that side's pattern selection.
- Page content is assembled by its own function — [Page content assembly](../invariant/page-content-assembly.md).
- Resource names resolve in the `/Resources` of the streams being executed, innermost first. `Tf` and `scn`/`SCN` put the resolved object in the graphics state. A font is resolved when first selected and cached by `FontKey` — [resource name scope](../invariant/resource-scope.md).
- `Tf` records the `FontKey` a name selected in the innermost scope (`ResourceScopes::select_font`) and reuses it until that scope is left. Measured 2026-09-30 on a local 392-page document at 72 dpi (minimum of 6 alternating runs): walking the scopes on every `Tf` rendered it in 9.64 s against 7.15 s before #127; with the per-scope record, 6.47 s against 6.49 s (10 runs).

## Code
- `justpdf-render/src/interpreter.rs` — `RenderInterpreter`, `render_page`, `get_page_content`, `concat_content_streams`, `execute_ops`, `execute_op`, `with_running_stream`, `with_stream_resources`, `select_font`, `resolve_font`, `resolve_xobject`, `effective_transform`, `cs_from_name`
- `justpdf-render/src/resources.rs` — `ResourceScopes`, `Resource`, `FontSelection`, `select_font`, `select_pattern`, `entry`
- `justpdf-render/src/graphics_state.rs` — `GraphicsState`, `TextState`, `FontKey`, `PatternSelection`, `Matrix`, `PdfBlendMode`, `fill_color_rgba`

## Reference behaviour
**None.** Clauses to compare against: ISO 32000-2 §8.4 (graphics state), Annex A. No record of comparing render results with MuPDF (example).

## Cross-cutting invariants
- [Page content assembly](../invariant/page-content-assembly.md)
- [Font resolution](../invariant/font-resolution.md) — `resolve_font` resolves fonts apart from text extraction.
- [resource name scope](../invariant/resource-scope.md)
- [Content stream recursion](../invariant/content-stream-recursion.md)

## Blast radius
- [Raster device](raster-device.md) — the only output target.
- [SVG renderer](svg-renderer.md), [BBox device](bbox-device.md) — copies of the same dispatch. When fixing operator handling, look at the copies too.
- [Render images](render-images.md), [Render clipping](render-clipping.md), [Render transparency](render-transparency.md), [Render shading](render-shading.md), [Render tiling patterns](render-tiling-patterns.md), [Glyph rendering](glyph-rendering.md), [Render annotations](render-annotations.md), [optional content](optional-content.md) — sub-concepts inside the same file.
- [Content stream parsing](content-stream-parsing.md) — input.
- [Color spaces](color-spaces.md) — the core module it bypasses.
- [Render API](render-api.md) — caller.

## Known holes / open
- `interpreter.rs` and `graphics_state.rs` have no unit tests. Render tests that read pixels are `tests/render_recursion.rs` (recursion paths) and `tests/render_resources.rs` (resource name scope); the other integration tests check only the PNG magic bytes and length.
- `gs` reads only `LW`, `LC`, `LJ`, `ML`, `ca`, `CA`, `BM` and `SMask`; the ExtGState `/Font` entry is ignored. Measured 2026-10-07: `/GS2 gs (ABC) Tj` with `/GS2` setting `/Font [<Courier> 12]` and no prior `Tf` draws nothing; after `/F1 12 Tf` it keeps F1. The `gs` font is an indirect reference that may be absent from `/Font`; `FontKey::Object` already keys the `fonts` cache by reference, so only a by-reference selection beside `select_font(name)` is missing (inferred). Not blocked by #207. Tracked: #238
- Nested stream boundaries have no floor on the graphics state stack, so an unbalanced `q`/`Q` crosses the boundary, and a path left after filling goes into the tile. Tracked: #128
