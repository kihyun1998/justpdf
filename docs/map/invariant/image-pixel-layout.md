# Image pixel layout

## The fact
A CMYK colour must convert to the same RGB (and the same gray) wherever justpdf converts it — in an image's samples, in a fill or stroke colour, and in the grayscale pass of compression. Today the conversion is the naive `(1 − c)(1 − k)`, `(1 − m)(1 − k)`, `(1 − y)(1 − k)`; whatever it becomes (an ICC-free improvement, rounding instead of truncation), every site must change with it, or the same document shows different colours rendered directly, rendered after compression, and rendered as an image versus as vector paint.

The rest of an image's sample layout — component count, bits per component, `/Decode`, array colour spaces — has one home since #46: `DecodedImage::to_rgb8` ([Image decoding](../territory/image-decoding.md)), and every consumer calls it (`image_to_rgba` in the raster and SVG renderers, `to_rgb_pixels` and the grayscale path in `writer/compress.rs`; `git grep -n 'to_rgb8()'` also lists `cbz` and `ocr`, but those call the `image` crate's `to_rgb8` on a `DynamicImage`, not this one — read 2026-10-31, #440). That part is no longer an invariant — it is a design rule of Image decoding. The 1-bit mask unpacking lives only in the raster renderer ([Render images](../territory/render-images.md)).

## Why it is cross-cutting
Three CMYK conversions exist, and none calls another (inferred, read at 88b2a08):
- `color/mod.rs — cmyk_to_rgb`, the one `Color::to_rgb` and the general path of `to_rgb8` use (fill and stroke colours, shadings, non-8-bit or `/Decode` images).
- The 8-bit `DeviceCMYK` fast path inside `image/mod.rs — DecodedImage::to_rgb8`, which repeats the formula inline and truncates to `u8`.
- `writer/compress.rs — cmyk_to_gray_from_operands`, which repeats the formula inline and then applies Rec.601 luminance, for `k`/`K` operators in the grayscale pass.

The shared assumption is the formula itself. Changing `cmyk_to_rgb` alone moves vector colours and leaves 8-bit CMYK images and grayscaled operators where they were.

## Territories it holds in
- [Color spaces](../territory/color-spaces.md) — `cmyk_to_rgb`, `Color::to_rgb`.
- [Image decoding](../territory/image-decoding.md) — the 8-bit CMYK fast path in `DecodedImage::to_rgb8`.
- [Compress grayscale](../territory/compress-grayscale.md) — `cmyk_to_gray_from_operands`.
- Consumers that inherit whichever copy they reach: [Render images](../territory/render-images.md), [SVG renderer](../territory/svg-renderer.md), [Render shading](../territory/render-shading.md) (`components_to_color` through `Color::to_rgb`), [Compress images](../territory/compress-images.md) (`to_rgb_pixels` through `to_rgb8`).

## What a violation looks like
A CMYK page whose vector fill and 8-bit image are the same colour in the file renders as two slightly different colours once one copy changes; a grayscaled page's text (operators) and photos (images) land on different grays for the same CMYK value. With only DeviceRGB and DeviceGray test inputs nothing shows, which is the usual state of the suite.

## Discovery history
First written 2026-09-23 as "interpretation lives in several copies" (sample layout, CMYK copies in the renderer, SVG, shading and compression), from a reading by two research agents. #46 and #231 moved sample interpretation into `to_rgb8` and colour conversion into `Color::to_rgb`. The 2026-10-30 rewrite found the three CMYK copies above still apart, and the maintainer chose to keep this note narrowed to them rather than fold it (2026-10-30).

## Where it will recur
**A function that converts CMYK components to RGB or gray.** Command: `rg -n '\(1\.0 - [cmyk]\) \* \(1\.0 - k\)' justpdf-*/src`. Call `color::cmyk_to_rgb` instead of writing the formula; a test converts one CMYK value as a fill colour, as an 8-bit image sample and through the grayscale pass and expects one result.
