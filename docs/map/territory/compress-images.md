# Compress images (re-encoding and downscaling)

## What it is
Decodes each image XObject, works out a target size from the largest size it is drawn at on a page (CTM), resizes it, and re-encodes it as an RGB JPEG. The image is replaced only when the JPEG is smaller than the stream's encoded bytes.

## Governing decisions
**None.** `dev/pdf-compress-wasm-design.md` §7 records the skips this stage applies (CMYK JPEG skipped for colour-conversion risk, images with an SMask skipped because JPEG has no alpha, inline images not handled, a larger result not replaced). It is a design doc, not a decision record; how far the code follows it is under Design model.

## Design model
- **Which images**: every stream in the modifier with `/Subtype /Image` (`recompress_images`), whether or not a page draws it. Inline images (`BI`…`EI`) are not touched.
- **Skipped** (`should_skip_image`, read at 88b2a08): an encoded stream smaller than `skip_below_bytes`; an `/ImageMask` stencil; an image with `/SMask`; an image whose `/ColorSpace` is the *name* `DeviceCMYK` or `CMYK`. `image_info` reads only a name for the colour space and assumes `DeviceRGB` for an array or a reference, so an `[/ICCBased N=4]` or a referenced CMYK space is not skipped as CMYK — it is converted to RGB and re-encoded (inferred). An image with a `/Mask` (colour-key array or explicit mask stream) is not skipped either (inferred).
- **Decoding** runs the whole filter chain before the codec (`image::decode_image`, #229), so a `[/FlateDecode /DCTDecode]` image is recompressed (`compression_recompresses_a_flate_wrapped_jpeg`). References inside the image dictionary resolve against the **original** `PdfDocument`, not the modifier (inferred from the `doc` argument).
- **Pixels become RGB through `DecodedImage::to_rgb8`** (#46), so Indexed, 1/2/4/16-bit and array-colour-space images keep their colours; `to_rgb_pixels` only resizes (Lanczos3). Before #46 a 16-bit RGB image panicked (`Invalid buffer length: expected 6 got 12`) and a 64 × 64 Indexed image was skipped (measured 2026-10-06, #46; pinned by `compressing_a_sixteen_bit_image_does_not_panic` and `a_compressed_indexed_image_keeps_its_colours`).
- **Target size** (`compute_target_dimensions_with_ctm`): with a measured display size, effective DPI is the larger of `pixels / (points / 72)` over the two axes; above `max_image_dpi` both axes scale down to it. An image drawn in several places takes **the largest width and the largest height seen**, each axis on its own (`collect_image_display_sizes`). With no measured size it falls back to a pixel budget: the longer side is capped at `max_dpi × 14` pixels when `max_dpi ≥ 72`; below 72 every image scales by `max_dpi / 72`.
- **Display sizes** are collected from page content only: `q`/`Q`/`cm` are tracked and each `Do` of a name in the page's own `/XObject` resources records the CTM's unit-square size (`collect_image_display_sizes`, `extract_xobject_map`, `multiply_matrix`). Forms, annotation appearances and inherited `/Resources` contribute nothing.
- **Quality**: `jpeg_quality`, or 75 when it is `None`. So setting only a DPI still re-encodes every eligible image.
- **The replacement dictionary is built new** with only `/Type`, `/Subtype`, `/Width`, `/Height`, `/BitsPerComponent 8`, `/ColorSpace /DeviceRGB`, `/Filter /DCTDecode` and `/Length`. Every other key of the original is lost: `/Decode`, `/Intent`, `/Interpolate`, `/Mask`, `/OC`, `/Metadata`, `/Alternates`, `/StructParent`. Measured 2026-10-30 (`justpdf compress --preset medium` on a hand-built 200 × 200 RGB image with `/Mask [0 10 0 10 0 10]`, `/OC` pointing at an OCG the default configuration turns off, and `/Interpolate true`): the output image dictionary holds none of the three. So a colour-key mask stops masking, and an image hidden by optional content becomes visible (inferred from the dictionary).
- **Input bounds**: nothing in this stage bounds an image's width, height or decoded size; it converts whatever `decode_image` returns ([Image decoding](image-decoding.md)). The image count is the number of image streams in the file.

## Code
- `justpdf-core/src/writer/compress.rs` — `recompress_images`, `should_skip_image`, `compute_target_dimensions_with_ctm`, `compute_target_dimensions`, `collect_image_display_sizes`, `multiply_matrix`, `extract_xobject_map`, `to_rgb_pixels`, `encode_jpeg_rgb`, `test_compress_medium_with_image`, `test_compress_high_downscale`, `test_compress_skip_small_image`, `test_safety_no_replace_if_larger`, `test_ctm_dpi_calculation`, `test_ctm_full_page_image`, `test_ctm_within_budget`, `test_ctm_fallback_no_display_size`, `test_multiply_matrix`
- `justpdf-core/tests/image_decode.rs` — `compression_recompresses_a_flate_wrapped_jpeg`
- `justpdf-render/tests/render_image_colour.rs` — `compressing_a_sixteen_bit_image_does_not_panic`, `a_compressed_indexed_image_keeps_its_colours`

## Reference behaviour
**None.** No record compares the result with Ghostscript's downsampling.

## Cross-cutting invariants
- [Image pixel layout](../invariant/image-pixel-layout.md) — samples become RGB through `DecodedImage::to_rgb8`.
- [Page content assembly](../invariant/page-content-assembly.md) — `collect_image_display_sizes` reads `/Contents` with its own copy of the stream-or-direct-array match.

## Blast radius
- [Image decoding](image-decoding.md) — this stage trusts `image_info` (name-only colour space, `is_mask`, `has_smask`) and the shape of `decode_image`'s output; a change there changes what is skipped and what is converted.
- [Content stream parsing](content-stream-parsing.md) — the display size depends on `cm` operands parsing as numbers; a non-number operand counts as 0.
- [Compress grayscale](compress-grayscale.md) — runs next on the JPEG this stage wrote, and builds its replacement dictionary the same way, so a rule changed on one side (a kept key, a skip) has to be changed on the other.
- [Compress dedup](compress-dedup.md) — runs later; two images that shared bytes before can be re-encoded to equal bytes and merged.
- [Optional content](optional-content.md) — the replacement drops `/OC`, so an image an OCG hides is drawn after compression.
- [Compress presets](compress-presets.md) — `jpeg_quality`, `max_image_dpi` and `skip_below_bytes` mean what this stage does with them.
- [Content interpreter](content-interpreter.md) — the walk that #139 and #300 decided to move display-size collection onto (Form recursion, CTM, form `/Resources`). The day this stage calls it, it inherits that interpreter's rules — [Content stream recursion](../invariant/content-stream-recursion.md) and [Resource scope](../invariant/resource-scope.md) — and what it does not report (images are not interpreted there today).
- [Render annotations](render-annotations.md), [Form flatten](form-flatten.md) — measuring `/AP` appearances (#300) means mapping each onto its `/Rect` as `annot::placement` does for both; a second mapping written here would disagree with them.

## Known holes / open
- The replacement dictionary drops `/Mask`, `/OC`, `/Decode`, `/Interpolate` and the other keys listed above (measured above). Tracked: #421.
- Only the *name* `DeviceCMYK` is skipped, so CMYK in an ICCBased or referenced space is converted to RGB, against the design doc's "CMYK JPEG — skip" (inferred). No issue tracks this.
- `compute_target_dimensions` is `#[allow(dead_code)]`; only `test_compute_target_dimensions_no_limit` and `test_compute_target_dimensions_with_limit` call it.
- A page whose `/Contents` is a reference to an array contributes no display sizes (inferred, same match as the other compress sites). Decided on #138: one compress-local helper reads `/Contents` for all four compress sites and reports content it cannot read. Tracked: #138.
- Sizes inside form XObjects do not count. Measured 2026-10-08 on #139 (`preset_high`, 1200 × 1200 gray image): drawn at 1 in on the page and 8 in inside a form, it is cut to 150 × 150 (18.75 dpi where the form draws it); drawn at 8 in inside a form only, it has no measured size, and the pixel-budget fallback keeps 1200 × 1200. Decided on #139: an image referenced from anything other than a page's own `/XObject` resources drops its page-measured size and takes the fallback (the guard font subsetting uses). The guard is not built for images: only font subsetting calls `object_referrers`/`fonts_reachable_outside_page_resources` (grep, 2026-10-30). Tracked: #139. Measuring sizes inside forms and `/AP` appearances narrows the guard. Tracked: #300. The shared content interpreter with a form hook that both issues waited for has landed (#207, closed 2026-10-08; `content/interpret.rs`, `InterpretOptions::enter_forms`); `compress.rs` does not call it yet (grep, 2026-10-30).
