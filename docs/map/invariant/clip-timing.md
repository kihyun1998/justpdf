# Clip timing

## The fact
`W` and `W*` do not clip anything when they run. They mark the current path to be intersected with the clipping path, and the intersection takes effect **after** the path-painting operator that ends the path (`S`, `f`, `B`, `n`, …), so that painting operator itself is not clipped by its own path. ISO 32000-2 §8.5.4 (cited from memory by the 2026-10-30 rewrite; not checked against the spec text). Check: `0 0 100 100 re W S` must stroke the whole outline, including the half of the stroke width that lies outside the rectangle.

## Why it is cross-cutting
Two content executors clip, and neither calls the other or a shared clip primitive: `RenderInterpreter` builds a tiny-skia mask and `SvgRenderer` writes a `<clipPath>`. Each decides on its own when `W` takes effect. Core's content interpreter does not clip at all, and the bbox device ignores `W`. A fix to the timing in one renderer leaves the other drawing a different picture of the same page.

## Territories it holds in
- [Render clipping](../territory/render-clipping.md) — the `"W"`/`"W*"` arms of `RenderInterpreter::execute_op` call `apply_clip` at once, which intersects `state.clip` with the current path before the painting operator runs (`justpdf-render/src/interpreter.rs — apply_clip`, `intersect_clip`; inferred, read at 88b2a08).
- [SVG renderer](../territory/svg-renderer.md) — the same arms call `SvgRenderer::apply_clip` at once, which pushes a `<clipPath>` and sets `active_clip_id`, so the stroke that follows is clipped (`justpdf-render/src/svg_device.rs — apply_clip`). It also replaces the active clip instead of intersecting it with the one before (measured 2026-10-30 by the map rewrite).

## What a violation looks like
`re W S` loses the outer half of the stroke: the clip is already the rectangle when `S` paints, so only the inner half of the line width shows (measured 2026-10-30 by the map rewrite, raster and SVG). A path that is filled and then used as a clip (`re W f`) shows no difference, which is why fill-only tests do not catch it; it shows with a stroke, or with `B`/`b`.

## Discovery history
Reported independently for both renderers by the render rewrite of the map on 2026-10-30, and measured there with a hand-made page. Tracked: #418. Made an invariant on the maintainer's answer the same day.

## Where it will recur
**Code that executes `W`/`W*` and owns a clip state.** Command: `rg -n '"W\*?"' justpdf-*/src`. A fix defers the intersection to the end of the next path-painting operator (a pending-clip flag set by `W`, applied after painting), in both renderers together; a test strokes a clipped rectangle and checks pixels outside the rectangle within half the line width.
