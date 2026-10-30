# BBox device

## What it is
Computes the bounding box of what a page draws, in PDF user space, without rendering pixels. It is a third operator walker, simpler than the raster and SVG interpreters, public as `compute_page_bbox`.

## Governing decisions
**None.** [ADR-0006](../../adr/0006-breaking-changes-are-allowed-before-1-0.md) and [the out-of-scope record on narrowing the render API](../../../.out-of-scope/narrowing-render-public-api.md) (2026-10-24, #371) keep `bbox_device` a public module before 1.0; they do not decide what it computes.

## Design model
Read from the source; no record states it.

- **It has its own content assembly** (`get_page_content`, `concat_streams`, commented "simplified version") **and its own operator walk** (`BBoxDevice::process_ops`) — [Page content assembly](../invariant/page-content-assembly.md).
- **The result is PDF user space.** `extend_point` maps a point through the CTM only, not the page transform, so `/Rotate` does not change the result. Measured 2026-10-07 (#262, closed as not reproduced): a `20 350 60 40 re f` fill on a 200×400 page gives 20,350–80,390 at `/Rotate 0` and `90`. Re-verified in the code at `88b2a08`.
- What extends the box (re-verified, `process_ops`):
  - a painted path: every point given to `m`, `l`, `c`, `v`, `y`, `re` (control points included), when a painting operator ends the path; `n` drops them;
  - text: for `Tj`, `'` and `TJ`, the text origin and a point `font_size` above it, with `Ts` added after the CTM. String width, `"`, `Tz`, `TL`, `T*` and the text render mode are not read, so invisible text (mode 3) counts;
  - `Do`: the unit square through the CTM, for an image and a Form XObject alike.
- Not read: `W`/`W*` (the box is not clipped), `sh`, `BI`, `gs`, optional content, and annotations.
- What bounds the input: nothing beyond the decoded content stream core returns (inferred).

## Code
- `justpdf-render/src/bbox_device.rs` — `BBoxDevice`, `BBoxDevice::new`, `BBoxDevice::bbox`, `BBoxDevice::process_ops`, `extend_point`, `extend_point_raw`, `extend_rect`, `compute_page_bbox`, `get_page_content`, `concat_streams`
- `justpdf-render/src/lib.rs` — `pub use bbox_device::compute_page_bbox`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Page content assembly](../invariant/page-content-assembly.md)

## Blast radius
- [Render interpreter](render-interpreter.md) — the operators this walk imitates; a change in how the raster side places text or forms does not reach this box.
- [Render API](render-api.md) — `compute_page_bbox` calls `compute_page_transform` for a value it never reads, so a change of that function's signature reaches it.

## Known holes / open
- `BBoxDevice::new` takes a `page_transform` and stores it, but nothing reads the field (`#[allow(dead_code)]`). The argument and field are dead; removing them would not change any result (inferred; #262).
- Public as `compute_page_bbox`, but nothing in the workspace or the bindings calls it (re-verified: `git grep compute_page_bbox` outside `justpdf-render/src` finds only `.out-of-scope/`).
- `Do` extends the box by the unit square through the CTM for a Form XObject too: the form's content, `/Matrix` and `/BBox` are not read (inferred, `process_ops`). The raster renderer's `/BBox` clip (#365) has no counterpart here.
- Text is a vertical segment at the text origin, not the glyphs' extent (inferred, above).
- A page index out of range is `RenderError::InvalidDimensions`, not `PageOutOfRange` (re-verified, `compute_page_bbox`). Decided in #150: look the page up with `get_page`, inheriting #134's `PageOutOfRange`/`InvalidObject` rule, wrapped in `RenderError::Core`. Tracked: #150
