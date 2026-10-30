# Render images

## What it is
Draws image XObjects, stencil image masks, an image's own `/SMask` and explicit `/Mask`, and (not yet) inline images onto the pixmap. XObject resolution (`resolve_xobject`) and the `Do` branch that splits into image and Form live here.

## Governing decisions
**None.**

## Design model
Read from the source and the issues cited; no decision record states it.

- **Decoding uses the single core `decode_image`**, and `image_to_rgba` only adds alpha 255 to `DecodedImage::to_rgb8`, the core conversion (#46) — [Image pixel layout](../invariant/image-pixel-layout.md).
- **Image XObjects, image masks, SMasks and explicit `/Mask` streams reach `decode_image` as their raw stream bytes**, decoded once there (#229; re-verified, `resolve_xobject` passes an image's `data` undecoded). Until #229 the renderer decoded them with `decode_stream` first: a Flate image drew nothing (the second decode failed and `do_xobject` dropped the error; measured 2026-09-30 and 2026-10-06, #229, 0 red pixels against MuPDF's 10000), a Flate SMask or `/Mask` dropped out and the image drew opaque (a red image under an all-0 Flate SMask drew 10000 red pixels, MuPDF 0), and a `[/FlateDecode /DCTDecode]` image drew only because that pre-decode inflated it. Pinned by `tests/render_images.rs`.
- **An image is the unit square through the CTM**, flipped so its first row is at the top, drawn at the fill alpha `ca` and the blend mode with bilinear sampling (re-verified, `render_image`; `/Interpolate` is not read).
- **A stencil mask (`/ImageMask true`) paints the fill colour where its bit says**, with `/Decode [1 0]` inverting it (only the first element is read). Its data is decoded through `decode_image` only when it has a `/Filter` (re-verified, `render_image_mask`).
- **1-bit masks ignore row padding**: the image-mask and explicit `/Mask` paths index the bits as one continuous run (`i / 8`), but each row of 1-bit data starts on a byte boundary (ISO 32000-2 §8.9.3). A mask whose width is not a multiple of 8 is sheared (measured 2026-10-06, #42: a 10×10 image mask painting the left 5 columns drew 3014 / 2264 red pixels on the left / right half of a 100×100 page; MuPDF 1.28.2 drew 5000 / 0. The 8×8 control drew 4700 / 0).
- A stencil mask unpacks its decoded data as 1-bit packing, but core's CCITT and JBIG2 paths return one byte per pixel (`bpc: 8`) (re-verified both sides: `render_image_mask`, `justpdf-core/src/image/mod.rs` `decode_image`; the mismatch is inferred, not measured).
- **An image's own `/SMask` and `/Mask` are read only through an indirect reference** to a stream (re-verified, `render_image`). The SMask's values multiply the image's alpha pixel for pixel, indexed by the image's pixel number, so an SMask of other dimensions is misapplied (inferred, `apply_image_smask`; `/Matte` is not read). The explicit `/Mask` is scaled to the image by nearest pixel. A colour-key `/Mask` array is not applied.
- The explicit-mask test asserts only that the masked and unmasked halves differ, not which half is painted: the polarity was not compared with a reference.
- **An image XObject with an `/SMask` or `/Mask` entry is drawn with the clip only**, without the graphics state's soft mask; other images and stencil masks draw through `draw_mask` (ISO 32000-1 §11.6.4.3, #129) — [Render transparency](render-transparency.md). The test is `dict.get(..).is_some()`, so a colour-key `/Mask` array, which is not applied, still drops the soft mask (inferred).
- **Inline images are not drawn**: the body of `render_inline_image` is a `TODO` and `Ok(())` (re-verified; measured 2026-10-06, #42: an inline 2×2 RGB image and an inline `/IM true` mask drew 0 pixels; MuPDF drew both).
- A broken image is skipped: `do_xobject` drops `render_image`'s error (re-verified).
- What bounds the input: nothing. `/Width` and `/Height` are read as `i64 as u32` by core `image_info` with no cap, `render_image_mask` allocates `w × h × 4` bytes, and `w * h` is a `u32` product that overflows for large images (inferred).

## Code
- `justpdf-render/src/interpreter.rs` — `do_xobject`, `resolve_xobject`, `XObjectData`, `render_image`, `render_image_mask`, `apply_image_smask`, `apply_image_explicit_mask`, `render_inline_image`, `image_to_rgba`
- `justpdf-render/tests/render_images.rs` — Flate images, SMasks and masks decoded once
- `justpdf-render/tests/render_image_colour.rs` — image colour conversion through the core

## Reference behaviour
- MuPDF 1.28.2 (measured 2026-10-06 for #42 and #229) — row padding of 1-bit masks, inline images, and Flate images, SMasks and masks.
- ISO 32000-2 §8.9.3 (row padding), §8.9.6 (masks); ISO 32000-1 §11.6.4.3 (an image's own mask overrides the soft mask, read raw for #129).

## Cross-cutting invariants
- [Image pixel layout](../invariant/image-pixel-layout.md)

## Blast radius
- [Image decoding](image-decoding.md) — the decoded layout (`bpc`, `components`, packed or one byte per pixel) is read here without a check; if it changes, `image_to_rgba` and both mask unpackers must follow.
- [Stream filters](stream-filters.md) — `decode_image` runs the filter chain; the raw-bytes rule (#229) means no filter may run twice.
- [Render transparency](render-transparency.md) — whether an image is masked by the graphics state's soft mask depends on its own `/SMask`/`/Mask`.
- [SVG renderer](svg-renderer.md), [Compress images](compress-images.md) — other readers of the same decoded output.
- [Document builder](document-builder.md) — PDFs whose page raster went in through `embed_rgb` (CBZ/SVG input, since #88) are Flate image XObjects; drawn since #229.

## Known holes / open
- **An image mask with `/Width 0` panics the renderer**: `render_image_mask` calls `IntSize::from_wh(w, h).unwrap()` on a zero size (measured 2026-10-30 with the CLI at `88b2a08`: `thread 'main' panicked at justpdf-render/src/interpreter.rs`, exit 101). `render_image` has the same `unwrap` on the decoded size (inferred). Tracked: #415.
- Inline images are not rendered, 1-bit masks ignore row padding, and stencil masks misread CCITT and JBIG2 output (above). Tracked: #42 (the double decode it also lists was fixed in #229)
- JPEG 2000 (`JPXDecode`) images never decode, so they are skipped. Tracked: #230
- A colour-key `/Mask`, `/Matte`, `/Interpolate` and an SMask of other dimensions are not handled (above, inferred).
- Image size is not bounded (above).
