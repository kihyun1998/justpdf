# Image decoding

## What it is
Takes an image XObject's dictionary and **raw** stream data and turns them into samples (`DecodedImage`), then into RGB (`DecodedImage::to_rgb8`). DCT goes through `jpeg-decoder` (`stream::dct::decode`), JPX through `justjp2`, JBIG2 through `justbig2`, CCITT through the core decoder, and anything else is the stream decode result as is. It is the one image decode path, shared by the raster renderer, the SVG renderer and the compressor.

## Governing decisions
**None.** Decided on an issue: #46 (maintainer, 2026-10-06) — `decode_image` keeps returning samples, and functions that need indirect objects take the document; `decode_image(raw, dict, doc)` since #229.

## Design model
Rules read from the code (*inferred* unless marked).
- **The codec is the last filter of the chain.** `decode_image` dispatches on the last `/Filter` name (`image_info`), runs every filter in order through `decode_stream_cow` (which passes DCT/JPX/JBIG2 through untouched), and hands the result to the codec (#229). `extract_jpeg_bytes` returns that same pre-codec output for a DCT image. Until #229 the raw bytes went straight to the codec: a `[/FlateDecode /DCTDecode]` JPEG failed with "first two bytes are not an SOI marker" and compression skipped it (*measured* 2026-10-06; pinned by `a_flate_wrapped_jpeg_decodes_like_the_bare_jpeg` in `tests/image_decode.rs`).
- **`/ColorSpace` is resolved** with `ColorSpace::resolve(obj, doc)`: a name, an array or a reference, and the references inside the array (#46). With no `/ColorSpace`, an image mask is DeviceGray and anything else is `Unknown`. `DecodedImage` carries the resolved space and the `/Decode` array, and a raw image's component count comes from the space (Indexed and Separation 1, ICCBased `/N`, DeviceN its names), falling back to `image_info`'s name-only count when the space reports 0. Codec paths keep the buffer's own count: DCT and JPX report theirs, and CCITT and JBIG2 give one 8-bit component (0/255) whatever `/BitsPerComponent` says. `image_info` still reports the name-only count (`num_components`, 3 for any array space); only the compressor's early skips read it.
- **`DecodedImage::to_rgb8` is the one place samples become RGB** for these callers ([Image pixel layout](../invariant/image-pixel-layout.md)). It unpacks 1, 2, 4, 8 and 16 bits per component, each row starting on a byte boundary, maps each sample through `/Decode` (default `[0 1]`, or `[0 2^bpc−1]` for an Indexed index, ISO 32000-1 §8.9.5.2), and converts with `Color::to_rgb`. A space whose component count differs from the buffer's is replaced by the device space with that many (`sample_space`). 8-bit DeviceGray, DeviceRGB and DeviceCMYK without `/Decode` take a fast path with the arithmetic the renderers used before (CMYK truncated, not rounded), so they convert exactly as before #46. A one-component image of at most 8 bits converts through a table of its 2^bpc values. `is_gray` tells whether the samples are gray already. Pinned by `tests/image_rgb.rs` and `justpdf-render/tests/render_image_colour.rs`; mutations dropping row padding, `/Decode`, the Indexed lookup, ICCBased `/N`, the tint transform, 16-bit reading, reference resolution or the fast path each failed a named test (#46).
  - Cost (*measured* 2026-10-24, release, best of 5, a 2000 × 2000 image filling a 400 × 400 pt page at 72 dpi, master → #46): 8-bit RGB 6.9 → 5.0 ms (fast path); Indexed 8-bit 2.8 → 7.1 ms; 1-bit gray 1.8 → 7.9 ms; CMYK with `/Decode [1 0 …]` 12.0 → 30.1 ms. master's figures are for wrong output (Indexed and 1-bit read as fewer RGB bytes, `/Decode` skipped). A first version converting every pixel through an allocated `Color` took 176 ms for the CMYK case; per-component decode tables, direct Gray/RGB/CMYK formulas and direct 8-bit reads brought it to 30 ms.
  - A real file changed (*measured* 2026-10-24, 18 local PDFs, 100 pages, pixel-compared against master): one page's `[/Indexed /DeviceRGB 255 <lookup stream>]` image (1565 × 1985, Flate) drew as black noise on master and draws as its score now. The other 99 pages are pixel-identical, and the optimised conversion renders all 100 identically to the first version.
- `read_sample` handles any `bpc` by shifting within one byte (`8 - bpc - bit % 8`), which is right only for 1, 2, 4 and 8; 16 has its own arm. A `/BitsPerComponent` outside those values (to_rgb8 clamps it to 1..16) underflows that shift: a panic in a debug build, wrong samples in release (inferred, not run).
- `ImageMask` sets `is_mask` (1 bit, default DeviceGray); `SMask` only sets `has_smask`. Neither is applied here: the renderer decodes a soft mask or mask as another image ([Render images](render-images.md)).
- JBIG2: `/JBIG2Globals` is read from the `/DecodeParms` entry at the JBIG2 filter's index, resolved through `doc` and decoded; an entry that does not resolve to a stream is an error, never a decode without it (`jbig2_globals`). Not verified against a real symbol-dictionary pair (no such fixture; the tests use an empty globals stream). The output is spread to 8-bit gray (1 → `0x00`).
- JPX: each component sample is clamped to 0..255 (`val.clamp(0, 255) as u8`), so the bit depth is ignored and samples above 8 bits saturate. `SMaskInData` is not handled.
- **Input bound:** the output buffers are sized from the file. `to_rgb8` allocates `width × height × 3` from `/Width` and `/Height` before reading samples; JPX and JBIG2 size theirs from the codec's reported dimensions (`(w * h) as usize`, `u32` arithmetic). Nothing caps them (#161 proposes an image limit from these values).

## Code
- `justpdf-core/src/image/mod.rs` — `ImageInfo`, `image_info`, `DecodedImage`, `DecodedImage::to_rgb8`, `DecodedImage::is_gray`, `DecodedImage::sample_space`, `DecodedImage::decode_ranges`, `read_sample`, `ImageFormat`, `decode_image`, `jbig2_globals`, `extract_jpeg_bytes`
- `justpdf-core/tests/image_rgb.rs` — `to_rgb8`: bits per component, row padding, `/Decode`, Indexed, ICCBased, Separation, DeviceN
- `justpdf-core/tests/image_decode.rs` — filter chains before the codec, `/JBIG2Globals`, compression of a Flate-wrapped JPEG

## Reference behaviour
- MuPDF 1.28.2, compared 2026-10-06 (#46): until #46 an Indexed image rendered white or gray; 1- and 4-bit gray images came out as stripes of intermediate grays; a 16-bit RGB image came out yellow and made `compress_pdf` panic; `/Decode [1 0]` did not invert; ICCBased `/N 1` and Separation images rendered white. MuPDF drew each correctly.
- MuPDF 1.28.2 decodes standard JPEG 2000 files that `justjp2` rejects (#230, 2026-10-06).
- Not compared: ISO 32000-2 §8.9 (images), §8.9.5 (Decode arrays), §11.6.5.3 (SMask).

## Cross-cutting invariants
- [Image pixel layout](../invariant/image-pixel-layout.md) — this function's output is where the invariant starts.

## Blast radius
- [Render images](render-images.md), [SVG renderer](svg-renderer.md) — call `decode_image` and `to_rgb8`. A change to the component count, bit depth or row layout of `DecodedImage` changes what each draws.
- [Compress images](compress-images.md), [Compress grayscale](compress-grayscale.md) — read the same output, and `image_info`'s name-only count for early skips; a change to either changes which images they touch.
- [Stream filters](stream-filters.md) — the other half of the pass-through rule: every filter before the codec must decode there, and the codec's own bytes must pass through untouched.
- [Color spaces](color-spaces.md) — `ColorSpace::resolve`, `substitute` and `Color::to_rgb` decide the RGB of every non-device image; a change there changes every converted pixel.

## Known holes / open
- JPX images never decode: `justjp2` rejects every standard JPEG 2000 file tried (*measured* 2026-10-06 on files made by Pillow/OpenJPEG; MuPDF decodes them). Which decoder to use is the maintainer's dependency decision. Tracked: #230
- JPX, CCITT and masks have no tests in `tests/image_decode.rs` or `tests/image_rgb.rs`.
- Callers of `decode_stream` get the codec's input bytes for DCT/JPX/JBIG2, not pixels; only `decode_image` produces pixels.
- Output buffers are sized from `/Width` × `/Height` and codec dimensions with no cap (above). Tracked: #161
