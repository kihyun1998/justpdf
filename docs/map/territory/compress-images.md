# Compress images (re-encoding and downscaling)

## What it is
Decodes image XObjects to RGB, computes a target DPI from the size each is actually drawn at on the page (CTM), shrinks them, and re-encodes them as JPEG. If the result is larger, the image is not replaced.

## Governing decisions
**None.**

## Design model
- Images with a mask or SMask, and CMYK images, are skipped (`should_skip_image`).
- An image used in several places is sized by **the largest size it is drawn at**. Without CTM information it falls back to a pixel budget of `max_dpi*14`.
- The replaced image dictionary is **built new** as DeviceRGB/DCTDecode — other keys of the original dictionary (Decode, Intent and so on) are lost.
- Without `jpeg_quality` it uses 75. So setting only a DPI still triggers re-encoding.
- Pixels become RGB through `DecodedImage::to_rgb8` (#46), so Indexed, 1/2/4/16-bit and array-colour-space images are recompressed in their colours; `to_rgb_pixels` only resizes. Until #46 a 16-bit RGB image panicked (`Invalid buffer length: expected 6 got 12`) and a 64 × 64 Indexed image was skipped (measured 2026-10-06; pinned by `render_image_colour.rs`).

## Code
- `justpdf-core/src/writer/compress.rs` — `recompress_images`, `should_skip_image`, `compute_target_dimensions_with_ctm`, `compute_target_dimensions`, `collect_image_display_sizes`, `multiply_matrix`, `extract_xobject_map`, `to_rgb_pixels`, `encode_jpeg_rgb`

## Reference behaviour
**None.** There is no record of comparing results with Ghostscript downsampling.

## Cross-cutting invariants
- [Image pixel layout](../invariant/image-pixel-layout.md) — a site that interprets `decode_image` output its own way.

## Blast radius
- [Image decoding](image-decoding.md) — if the output shape of `image_info`/`decode_image` changes, this breaks first.
- [Content stream parsing](content-stream-parsing.md) — CTM collection relies on operator parsing.
- [Compress dedup](compress-dedup.md) — runs after replacement. Images that shared one source can each be re-encoded and then merged.
- [Compress grayscale](compress-grayscale.md) — has a separate image path (fixed q65). When changing the rules on one side, check the other.
- [Compress presets](compress-presets.md) — `jpeg_quality`/`max_image_dpi`/`skip_below_bytes`.

## Known holes / open
- `compute_target_dimensions` is `#[allow(dead_code)]` and only tests call it.
- `collect_image_display_sizes` reads page content only, so the size an image is drawn at inside a form XObject does not count toward its target size. Measured 2026-10-08 (`preset_high`, 1200×1200 gray image): drawn at 1 in on the page and 8 in inside a form, it is cut to 150×150 (18.75 dpi where the form draws it); drawn at 8 in inside a form only, it has no measured size and the pixel-budget fallback keeps 1200×1200. Decided in #139: an image referenced from anything other than a page's own `/XObject` resources drops its page-measured size and takes the fallback (the guard font subsetting uses). Tracked: #139. Measuring sizes inside forms and `/AP` appearances, through the #207 Form hook, then narrows the guard. Tracked: #300.
