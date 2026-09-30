# Render images

## What it is
Draws image XObjects, image masks, SMasks, explicit masks and inline images onto the pixmap. XObject resolution (`resolve_xobject`) and the `Do` branch split into image and Form here.

## Governing decisions
**None.**

## Design model
- Decoding uses the single core `decode_image`, then its own `image_to_rgba` interprets the result: components 1/3/4 only, always 8-bit, simple CMYK — [Image pixel layout](../invariant/image-pixel-layout.md).
- An image mask reads its result unpacked from 1-bit packing, but the CCITT and JBIG2 paths return one byte per pixel (inferred: mismatch).
- **SMasks and explicit masks are decoded twice**: after decoding with `self.doc.decode_stream`, the result is passed again to `decode_image` together with the same dictionary. A Flate mask fails the second decode and the mask silently drops out (inferred).
- **Inline images are not drawn**: the body of `render_inline_image` is only `// TODO` and `Ok(())`.
- `resolve_xobject` passes DCTDecode as the raw bytes and everything else through `decode_stream`.
- **The image XObject itself is decoded twice too**: `render_image` hands the bytes `resolve_xobject` already decoded to `decode_image` with the same dictionary. A Flate image fails the second decode and `do_xobject` drops the error, so nothing is drawn (measured 2026-09-30 with SVG `to_pdf` output — see [SVG input](svg-input.md)). Unfiltered images are unaffected; other filters not checked.

## Code
- `justpdf-render/src/interpreter.rs` — `do_xobject`, `resolve_xobject`, `render_image`, `render_image_mask`, `apply_image_smask`, `apply_image_explicit_mask`, `render_inline_image`, `image_to_rgba`

## Reference behaviour
**None.** Clauses to compare against: ISO 32000-2 §8.9, §8.9.6 (masks), §11.6.5.3.

## Cross-cutting invariants
- [Image pixel layout](../invariant/image-pixel-layout.md)

## Blast radius
- [Image decoding](image-decoding.md) — input. If its output shape changes, look at `image_to_rgba` and the mask unpacking.
- [Stream filters](stream-filters.md) — the cause of the double decode (the pass-through rule).
- [Render transparency](render-transparency.md) — applying SMasks.
- [SVG renderer](svg-renderer.md), [compress-images](compress-images.md) — other interpreters of the same decoded output.
- [Document builder](document-builder.md) — PDFs whose page raster went in through `embed_rgb` (CBZ/SVG input, since #88) are Flate image XObjects and are not drawn here (above).

## Known holes / open
- Inline images are not rendered (above). Inline images are only parsed.
- Tracked: #42 (render inline images and masks)
