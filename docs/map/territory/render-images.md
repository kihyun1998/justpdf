# Render images

## What it is
Draws image XObjects, image masks, SMasks, explicit masks and inline images onto the pixmap. XObject resolution (`resolve_xobject`) and the `Do` branch split into image and Form here.

## Governing decisions
**None.**

## Design model
- Decoding uses the single core `decode_image`, then its own `image_to_rgba` interprets the result: components 1/3/4 only, always 8-bit, simple CMYK — [Image pixel layout](../invariant/image-pixel-layout.md).
- An image mask reads its result unpacked from 1-bit packing, but the CCITT and JBIG2 paths return one byte per pixel (inferred: mismatch).
- **1-bit masks ignore row padding**: the image-mask and explicit `/Mask` paths index the bits as one continuous run (`i / 8`), but each row of 1-bit data starts on a byte boundary (ISO 32000-2 §8.9.3). A mask whose width is not a multiple of 8 is sheared (measured 2026-10-06: a 10×10 image mask painting the left 5 columns drew 3014 / 2264 red pixels on the left / right half of a 100×100 page; MuPDF 1.28.2 drew 5000 / 0. The 8×8 control drew 4700 / 0).
- **SMasks and explicit masks are decoded twice**: after decoding with `self.doc.decode_stream`, the result is passed again to `decode_image` together with the same dictionary. A Flate mask fails the second decode and the mask silently drops out, so the image draws fully opaque (measured 2026-10-06 for an SMask: a red image under an all-0 Flate SMask drew 10000 red pixels, MuPDF 0; the unfiltered SMask control drew 0. Explicit `/Mask` inferred from the same code).
- The root of every double decode: `decode_image` takes the stream's **raw** bytes and applies the filters itself; the renderer decodes before calling it. The image-mask path works only by accident, falling back to the once-decoded bytes when the second decode fails.
- **Inline images are not drawn**: the body of `render_inline_image` is only `// TODO` and `Ok(())` (measured 2026-10-06: an inline 2×2 RGB image and an inline `/IM true` mask drew 0 pixels; MuPDF drew both).
- `resolve_xobject` passes DCTDecode as the raw bytes and everything else through `decode_stream`.
- **The image XObject itself is decoded twice too**: `render_image` hands the bytes `resolve_xobject` already decoded to `decode_image` with the same dictionary. A Flate image fails the second decode and `do_xobject` drops the error, so nothing is drawn (measured 2026-09-30 with SVG `to_pdf` output — see [SVG input](svg-input.md)). Unfiltered images are unaffected; other filters not checked. Re-measured 2026-10-06 on a hand-built 10×10 Flate RGB image: 0 red pixels, MuPDF 10000.

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
- [SVG renderer](svg-renderer.md), [Compress images](compress-images.md) — other interpreters of the same decoded output.
- [Document builder](document-builder.md) — PDFs whose page raster went in through `embed_rgb` (CBZ/SVG input, since #88) are Flate image XObjects and are not drawn here (above).

## Known holes / open
- Inline images are not rendered (above). Inline images are only parsed.
- Tracked: #42 (double decode, 1-bit mask row padding, mask shape, inline images; its brief also covers the SVG renderer's double decode), #46 (`image_to_rgba` bits per component and array color spaces)
