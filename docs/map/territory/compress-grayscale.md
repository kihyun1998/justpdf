# Compress grayscale

## What it is
Turns images into grayscale JPEG (fixed quality 65) and rewrites the color operators in page content streams as gray operators.

## Governing decisions
**None.**

## Design model
- Only `rg`/`RG`/`k`/`K` are rewritten. `sc`/`SC`/`scn`/`SCN` are left as they are.
- Only the page's `/Contents` is rewritten. Colors inside form XObjects stay as they are.
- Operators that are not color operators are rewritten with `ContentOp::write_to` — they read back the same (`test_grayscale_keeps_other_operators_unchanged`, #29). Before #29 its own `write_operand` did not escape names, rounded reals to 4 decimal places, and left only `"BI "` of an inline image. New gray values are written with `write_gray` — rounded to 4 decimal places, then through object serialization's `write_real` (always a decimal point, NaN → `0.0`, ±Inf → ±`f32::MAX`). Before #29 a value close to an integer was written as an integer and anything else with `{:.4}`, so an inf input became `inf g` — [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md).
- Images are turned gray from `DecodedImage::to_rgb8` with the Rec.601 weights (#46). An image already gray (`DecodedImage::is_gray`: one component in DeviceGray, CalGray or an ICCBased standing in for one) is left as it is; a one-component colour image (Indexed, Separation) is converted. Until #46 the pixels were grouped by the name-only component count, so an array colour space was read as RGB.
- The gray weights are Rec.601, while the renderer's soft mask uses Rec.709 — the two sites use different luminance formulas.
- No preset turns this step on ([Compress presets](compress-presets.md)).

## Code
- `justpdf-core/src/writer/compress.rs` — `convert_images_to_grayscale`, `encode_jpeg_gray`, `rewrite_color_operators_to_gray`, `rgb_to_gray_from_operands`, `cmyk_to_gray_from_operands`, `write_gray`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md) — `ContentOp::write_to`.
- [Image pixel layout](../invariant/image-pixel-layout.md) — the image path.

## Blast radius
- [Content stream parsing](content-stream-parsing.md) — parses, then rewrites with that side's `write_to`.
- [Compress images](compress-images.md) — a separate image encoding path.
- [Render transparency](render-transparency.md) — the other side of the luminance formula mismatch.

## Known holes / open
- `test_grayscale_conversion_reduces_size` has a conditional exit, `if stats.images_grayscaled > 0`, so it passes even when nothing is converted.
