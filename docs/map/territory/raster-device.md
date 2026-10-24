# Raster device (tiny-skia)

## What it is
`PixmapDevice` fills paths, strokes lines, and draws images and patterns onto a tiny-skia pixmap, draws through the mask each call is given, and encodes to PNG/JPEG/RGBA.

## Governing decisions
**None.**

## Design model
- **It holds no clip** (#50): every drawing method takes `clip: Option<&Mask>`, which the interpreter computes from the graphics state. `clip_mask` builds a fresh pixmap-sized mask for a path. Until #50 the device held the clip in a `pub(crate)` field that the interpreter rewrote when applying a soft mask.
- `encode_png` exists only on this internal type. The public return types (`RenderedPixmap`, `Vec<u8>`) do not have it (the README example calls it — [Published docs](published-docs.md)).

## Code
- `justpdf-render/src/device.rs` — `PixmapDevice`, `fill_path`, `stroke_path`, `draw_image`, `draw_pixmap`, `fill_path_with_pattern`, `clip_mask`, `encode_png`, `encode_jpeg`, `raw_rgba`

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
