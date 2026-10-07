# BBox device

## What it is
Computes the bounding box of what is actually drawn on a page. It is the third operator walker (a simplified version).

## Governing decisions
**None.**

## Design model
- Has its own content assembly ("simplified version") and operator walk.

## Code
- `justpdf-render/src/bbox_device.rs` — `BBoxDevice`, `process_ops`, `get_page_content`, `concat_streams`, `compute_page_bbox`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Page content assembly](../invariant/page-content-assembly.md)

## Blast radius
- [Render interpreter](render-interpreter.md) — the original dispatch.

## Known holes / open
- `BBoxDevice::new` takes a `page_transform` and stores it, but nothing reads the field: `add_point` applies the CTM only, so the result is PDF user space (measured 2026-10-07, #262). The argument and field are dead; removing them would not change any result.
- Public as `compute_page_bbox`, but nothing in the workspace or the bindings calls it.
- Tracked: #150 (a page index out of range is `RenderError::InvalidDimensions`)
