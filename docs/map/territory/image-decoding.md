# Image decoding

## What it is
Takes an image XObject's dictionary (`image_info`) and **raw** stream data and turns them into pixels (`DecodedImage`). DCT goes through `jpeg-decoder`, JPX through `justjp2`, JBIG2 through `justbig2`, CCITT through the core decoder, and everything else is the stream decode result as is. It is the **only** image decode path, shared by the renderer and the compressor.

## Governing decisions
- #46 (maintainer, 2026-10-06): `decode_image` keeps returning samples, and functions that need indirect objects take the document — `decode_image(raw, dict, doc)` since #229.

## Design model
- The codec is the **last filter** of the chain. Every filter runs in order through `decode_stream`, which passes DCT/JPX/JBIG2 through untouched, and the codec gets the result (#229); `extract_jpeg_bytes` returns that same pre-codec output for a DCT image. Until #229 the raw bytes went straight to the codec: a `[/FlateDecode /DCTDecode]` JPEG failed with "first two bytes are not an SOI marker" and compression skipped it (measured 2026-10-06).
- **`/ColorSpace` is resolved** with `ColorSpace::resolve(obj, doc)`: a name, an array or a reference, with the references inside the array (#46). `DecodedImage` carries the resolved space and the `/Decode` array, and a raw image's component count comes from the space (Indexed and Separation 1, ICCBased `/N`, DeviceN its names). Codec paths keep the buffer's own count: DCT and JPX report theirs, and CCITT and JBIG2 give one 8-bit component (0/255) whatever `/BitsPerComponent` says. `image_info` still reports the name-only count (inferred: only the compressor's early skips read it).
- **`DecodedImage::to_rgb8` is the one place samples become RGB** ([Image pixel layout](../invariant/image-pixel-layout.md)). It unpacks 1, 2, 4, 8 and 16 bits per component, each row starting on a byte boundary, maps each sample through `/Decode` (default `[0 1]`, or `[0 2^bpc−1]` for an Indexed index, ISO 32000-1 §8.9.5.2), and converts with `Color::to_rgb`. A space whose component count differs from the buffer's is replaced by the device space with that many. 8-bit DeviceGray, DeviceRGB and DeviceCMYK without `/Decode` take a fast path with the arithmetic the renderers used before (CMYK truncated), so they convert exactly as before #46. A one-component image of at most 8 bits converts through a table of its 2^bpc values. `is_gray` tells whether the samples are gray already.
  - Cost (measured 2026-10-24, release, best of 5, a 2000 × 2000 image filling a 400 × 400 pt page at 72 dpi, master → #46): 8-bit RGB 6.9 → 5.0 ms (fast path); Indexed 8-bit 2.8 → 7.1 ms; 1-bit gray 1.8 → 7.9 ms; CMYK with `/Decode [1 0 …]` 12.0 → 30.1 ms. master's figures are for wrong output (Indexed and 1-bit read as fewer RGB bytes, `/Decode` skipped). A first version converting every pixel through an allocated `Color` took 176 ms for the CMYK case. Per-component decode tables, direct Gray/RGB/CMYK formulas and direct 8-bit reads brought it to 30 ms.
  - A real file changed (measured 2026-10-24, 18 local PDFs, 100 pages, pixel-compared against master): one page's `[/Indexed /DeviceRGB 255 <lookup stream>]` image (1565 × 1985, Flate) drew as black noise on master and draws as its score now. The other 99 pages are pixel-identical, and the optimised conversion renders all 100 identically to the first version.
  - Until #46 (measured 2026-10-06 against MuPDF 1.28.2): an Indexed image rendered white or gray; 1- and 4-bit gray images came out as stripes of intermediate grays; a 16-bit RGB image came out yellow and made `compress_pdf` panic; `/Decode [1 0]` did not invert; ICCBased `/N 1` and Separation images rendered white. Pinned by `justpdf-core/tests/image_rgb.rs` and `justpdf-render/tests/render_image_colour.rs`; mutations dropping row padding, `/Decode`, the Indexed lookup, ICCBased `/N`, the tint transform, 16-bit reading, reference resolution or the fast path each fail a named test.
- `SMask`/`ImageMask` only set a flag.
- JBIG2: `/JBIG2Globals` is read from the `/DecodeParms` entry at the JBIG2 filter's index, resolved through `doc` and decoded; an entry that does not resolve to a stream is an error, never a decode without it. Not verified against a real symbol-dictionary pair (no such fixture; the tests use an empty globals stream). The output is spread out to 8-bit gray (1 → 0x00).
- JPX: components are truncated i32 → u8 and the bit depth is ignored. `SMaskInData` is not handled.
- What the output pixels mean (component count, bit depth) is interpreted differently by each caller — [Image pixel layout](../invariant/image-pixel-layout.md).

## Code
- `justpdf-core/tests/image_rgb.rs` — `to_rgb8`: bits per component, row padding, `/Decode`, Indexed, ICCBased, Separation, DeviceN
- `justpdf-core/src/image/mod.rs` — `DecodedImage::to_rgb8`, `is_gray`, `ImageInfo`, `image_info`, `DecodedImage`, `ImageFormat`, `decode_image`, `extract_jpeg_bytes`

## Reference behaviour
**None.** Clauses to compare against: ISO 32000-2 §8.9 (images), §8.9.5 (Decode arrays), §11.6.5.3 (SMask).

## Cross-cutting invariants
- [Image pixel layout](../invariant/image-pixel-layout.md) — this function's output is where the invariant starts.

## Blast radius
- [Render images](render-images.md), [SVG renderer](svg-renderer.md) — consumers of `decode_image`. If the output shape changes, check each one's `image_to_rgba`.
- [Compress images](compress-images.md), [Compress grayscale](compress-grayscale.md) — other interpreters of the same output.
- [Stream filters](stream-filters.md) — the other half of the pass-through rule.
- [Color spaces](color-spaces.md) — `ColorSpace::resolve` and `Color::to_rgb`.

## Known holes / open
- `justpdf-core/tests/image_decode.rs` covers filter chains and `/JBIG2Globals`; JPX, CCITT and masks still have no tests here.
- JPX and JBIG2 pass through the stream layer as raw bytes and are decoded to pixels only in the image layer. Callers of `decode_stream` do not get pixels.
