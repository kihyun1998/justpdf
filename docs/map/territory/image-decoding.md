# Image decoding

## What it is
Takes an image XObject's dictionary (`image_info`) and data and turns them into pixels (`DecodedImage`). DCT goes through `jpeg-decoder`, JPX through `justjp2`, JBIG2 through `justbig2`, CCITT through the core decoder, and everything else is the stream decode result as is. It is the **only** image decode path, shared by the renderer and the compressor.

## Governing decisions
**None.**

## Design model
- The branch is picked by the **last filter** of the filter chain, and the raw bytes are handed straight to that decoder. With a chain like `[/FlateDecode /DCTDecode]`, compressed bytes go to the JPEG decoder (inferred).
- `/ColorSpace` is read only when it is a name. Arrays and references (Indexed, ICCBased and so on) fall back to DeviceRGB with 3 components. It does not use `from_pdf_object` from [Color spaces](color-spaces.md).
- `/Decode` is not applied. `SMask`/`ImageMask` only set a flag.
- JBIG2: `JBIG2Globals` is not read. The output is spread out to 8-bit gray (1 → 0x00).
- JPX: components are truncated i32 → u8 and the bit depth is ignored. `SMaskInData` is not handled.
- What the output pixels mean (component count, bit depth) is interpreted differently by each caller — [Image pixel layout](../invariant/image-pixel-layout.md).

## Code
- `justpdf-core/src/image/mod.rs` — `ImageInfo`, `image_info`, `DecodedImage`, `ImageFormat`, `decode_image`, `extract_jpeg_bytes`

## Reference behaviour
**None.** Clauses to compare against: ISO 32000-2 §8.9 (images), §8.9.5 (Decode arrays), §11.6.5.3 (SMask).

## Cross-cutting invariants
- [Image pixel layout](../invariant/image-pixel-layout.md) — this function's output is where the invariant starts.

## Blast radius
- [Render images](render-images.md), [SVG renderer](svg-renderer.md) — consumers of `decode_image`. If the output shape changes, check each one's `image_to_rgba`.
- [Compress images](compress-images.md), [Compress grayscale](compress-grayscale.md) — other interpreters of the same output.
- [Stream filters](stream-filters.md) — the other half of the pass-through rule.
- [Color spaces](color-spaces.md) — supporting array color spaces means using `from_pdf_object` here.

## Known holes / open
- `decode_image` has no unit tests (JPX/JBIG2/CCITT/masks, all of them).
- JPX and JBIG2 pass through the stream layer as raw bytes and are decoded to pixels only in the image layer. Callers of `decode_stream` do not get pixels.
- Tracked: #46 (image decode color space and Decode)
