# Raster device (tiny-skia)

## What it is
`PixmapDevice` owns the page's tiny-skia pixmap. It fills paths, strokes lines, draws images, pixmaps and pattern fills onto it through the mask each call is given, and encodes the result to PNG, JPEG or raw RGBA.

## Governing decisions
**None.** [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) makes `tiny-skia` this crate's dependency layer; it does not decide the device's shape.

## Design model
Read from the source; no record states it.

- **It holds no clip** (#50). Every drawing method takes `clip: Option<&Mask>`, which the interpreter computes from the graphics state (`draw_mask`). `clip_mask` builds a fresh pixmap-sized mask for a path (re-verified, `device.rs`). Until #50 the device held the clip in a `pub(crate)` field that the interpreter rewrote when applying a soft mask.
- **The pixmap is a public field**, and the interpreter uses it directly: groups, soft masks and tiles swap their own pixmap into `PixmapDevice::pixmap`, and `shading::render_shading` writes pixels into it (re-verified, `interpreter.rs`). The device's methods are not the only way pixels change.
- Every fill and stroke is antialiased; images, pixmaps and patterns are sampled `FilterQuality::Bilinear`; pattern shaders repeat (`SpreadMode::Repeat`) (re-verified, `device.rs`).
- A stroke takes width, cap, join, miter limit and dash from the `GraphicsState` it is given (`stroke_path`, `stroke_path_with_pattern`).
- `encode_png`, `encode_jpeg` and `raw_rgba` exist only on this type. The public results of [Render API](render-api.md) are `Vec<u8>` and `RenderedPixmap`.
- **Pixel data is premultiplied.** `raw_rgba` returns tiny-skia's pixmap data, which is premultiplied RGBA; `encode_jpeg` copies its RGB and drops alpha (inferred, from tiny-skia 0.11's `Pixmap::data` and `encode_jpeg`). With the default opaque background the two are the same.
- What bounds the input: the pixmap size is set by its caller; `render_to_device` caps it at 16384 px per side ([Render API](render-api.md)). `PixmapDevice::new` fails with `RenderError::InvalidDimensions` only where tiny-skia 0.11.4 `Pixmap::new` refuses the size: a side of 0, a row of more than `i32::MAX` bytes, or a byte length overflowing `usize` (`min_row_bytes`, `compute_data_len`). It does not cap the total size, and it allocates with `vec!`, so a size it accepts but memory cannot hold aborts the process (inferred, read in the local crate on #426).

## Code
- `justpdf-render/src/device.rs` — `PixmapDevice`, `PixmapDevice::new`, `clip_mask`, `fill_path`, `stroke_path`, `draw_image`, `draw_pixmap`, `fill_path_with_pattern`, `stroke_path_with_pattern`, `clear`, `encode_png`, `encode_jpeg`, `raw_rgba`, `dimensions`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Render interpreter](render-interpreter.md) — swaps and writes `pixmap` directly, so it assumes one page-sized tiny-skia pixmap behind the device.
- [Render shading](render-shading.md) — mesh and radial shadings write premultiplied pixels into the pixmap themselves and assume tiny-skia's pixel layout.
- [Render clipping](render-clipping.md), [Render transparency](render-transparency.md) — every mask they hand over must be the pixmap's size; a mask of another size draws nothing.
- [Render API](render-api.md) — its output formats are the encoders here, and their premultiplied or alpha-dropping output.

## Known holes / open
- `device.rs` has no unit tests; it is exercised through the render integration tests.
- With a background whose alpha is below 255, `OutputFormat::RawRgba` and `render_page_to_pixmap` return premultiplied values although `OutputFormat::RawRgba`'s doc says "Raw RGBA pixel data", and JPEG writes the premultiplied colour (inferred, not measured).
