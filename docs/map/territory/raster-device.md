# Raster device (tiny-skia)

## What it is
`PixmapDevice` fills paths, strokes lines, and draws images and patterns onto a tiny-skia pixmap, holds the clip mask, and encodes to PNG/JPEG/RGBA.

## Governing decisions
**None.**

## Design model
- `clip_mask` is `pub(crate)` and the interpreter changes it directly when applying a soft mask — the interpreter crosses the device boundary.
- `encode_png` exists only on this internal type. The public return types (`RenderedPixmap`, `Vec<u8>`) do not have it (the README example calls it — [Published docs](published-docs.md)).

## Code
- `justpdf-render/src/device.rs` — `PixmapDevice`, `fill_path`, `stroke_path`, `draw_image`, `draw_pixmap`, `fill_path_with_pattern`, `set_clip_path`, `intersect_clip_path`, `clear_clip`, `encode_png`, `encode_jpeg`, `raw_rgba`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Render interpreter](render-interpreter.md) — the only caller.
- [Render clipping](render-clipping.md), [Render transparency](render-transparency.md) — share the clip mask.
- [Render API](render-api.md) — the encoded output.

## Known holes / open
- No tests.
