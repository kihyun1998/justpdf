# Compress grayscale

## What it is
Turns colour image XObjects into grayscale JPEGs at a fixed quality of 65, and rewrites the RGB and CMYK colour operators in page content streams as gray operators.

## Governing decisions
**None.** `dev/pdf-compress-wasm-design.md` §7 records that grayscale is reached only through an option flag and no preset turns it on; the code follows it (`preset_*`, read at 88b2a08). It is a design doc, not a decision record.

## Design model
- **Images** (`convert_images_to_grayscale`): every stream with `/Subtype /Image`, after image recompression has run, so a JPEG that stage just wrote is decoded and re-encoded again. Skipped: a colour space whose name is `DeviceGray`, an `/ImageMask`, an image with `/SMask`, an image `decode_image` cannot decode, and an image already gray by `DecodedImage::is_gray` (one component in DeviceGray, CalGray or an ICCBased standing in for one). Unlike [Compress images](compress-images.md) it does **not** skip CMYK, ignores `skip_below_bytes`, and does not downscale (inferred, read at 88b2a08).
- Pixels go through `DecodedImage::to_rgb8`, then Rec.601 weights (`0.299 R + 0.587 G + 0.114 B`) (#46). A one-component colour image (Indexed, Separation) is converted. Until #46 the pixels were grouped by the name-only component count, so an array colour space was read as RGB.
- The image is replaced only when the gray JPEG is smaller than the stream's encoded bytes. The replacement dictionary is built new (`/ColorSpace /DeviceGray`, `/Filter /DCTDecode`, size keys), so it drops `/Mask`, `/OC`, `/Decode`, `/Interpolate` and the rest, as image recompression does.
- **Content** (`rewrite_color_operators_to_gray`): only `rg`/`RG`/`k`/`K` are rewritten, to `g`/`G`; CMYK goes to RGB as `(1 − c)(1 − k)` and so on, then Rec.601. `sc`/`SC`/`scn`/`SCN`, `cs`/`CS`, shadings and inline images are left as they are. Only streams a page's `/Contents` names are rewritten; colours inside form XObjects, patterns and annotation appearances stay.
- A rewritten stream is re-encoded with Flate at the best level whatever its filter was (`DecodeParms` dropped), and is written even if larger. A stream with no colour operator is left byte-identical.
- Operators other than the four are written back with `ContentOp::write_to`, so they read back the same (`test_grayscale_keeps_other_operators_unchanged`, #29). Before #29 its own `write_operand` did not escape names, rounded reals to 4 decimal places, and kept only `"BI "` of an inline image.
- New gray values are written by `write_gray`: rounded to 4 decimal places, then through `object::write_real` (always a decimal point, NaN → `0.0`, ±Inf → ±`f32::MAX`) (`test_gray_values_read_back_as_reals`). Before #29 a value close to an integer was written as an integer and others with `{:.4}`, so an infinite input became `inf g` — [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md).
- **Each `/Contents` stream is parsed on its own**, not joined with its siblings. The parser drops operands left over at the end of a stream (`ContentParser::next_op`, "Leftover operands without operator"), so when an operator's operands sit in one stream and the operator in the next, rewriting the first stream loses them (inferred, read at 88b2a08; it bites only a stream that also holds one of the four operators).
- The gray weights are Rec.601, while the renderer's luminosity soft masks use Rec.709 (`0.2126`, `0.7152`, `0.0722` in `apply_image_smask` and `pixmap_to_mask`, `justpdf-render/src/interpreter.rs`).
- **Input bounds**: none of its own; it handles every image stream and every page content stream, at whatever size they decode to.

## Code
- `justpdf-core/src/writer/compress.rs` — `convert_images_to_grayscale`, `encode_jpeg_gray`, `rewrite_color_operators_to_gray`, `operand_to_f64`, `rgb_to_gray_from_operands`, `cmyk_to_gray_from_operands`, `write_gray`, `pdf_with_referenced_color_space`, `test_grayscale_conversion_reduces_size`, `test_grayscale_roundtrip_valid`, `test_grayscale_leaves_images_with_referenced_color_space`, `test_gray_values_read_back_as_reals`, `test_grayscale_keeps_other_operators_unchanged`, `test_grayscale_disabled`, `test_grayscale_text_only_noop`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md) — `ContentOp::write_to` and `write_real`.
- [Image pixel layout](../invariant/image-pixel-layout.md) — the image path reads samples through `DecodedImage::to_rgb8`.
- [Page content assembly](../invariant/page-content-assembly.md) — `rewrite_color_operators_to_gray` matches `/Contents` as a reference or a direct array, and parses each stream separately instead of joining them.

## Blast radius
- [Content stream parsing](content-stream-parsing.md) — the rewrite is only as faithful as `parse_content_stream` plus `ContentOp::write_to`; an operand the parser drops or a form `write_to` changes is lost from the page.
- [Compress images](compress-images.md) — runs first and writes the JPEG this stage re-decodes; both build the replacement dictionary from scratch, so a key kept or a skip added on one side has to be added on the other.
- [Render transparency](render-transparency.md) — the other side of the luminance formula mismatch (Rec.709 there, Rec.601 here).
- [Compress presets](compress-presets.md) — no preset sets `grayscale`; only WASM `compress_advanced`, the CLI and callers do.

## Known holes / open
- `test_grayscale_conversion_reduces_size` checks the size only `if stats.images_grayscaled > 0`, so it passes when nothing is converted.
- `test_grayscale_leaves_images_with_referenced_color_space` adds its colour space through the modifier, so the reference does not resolve in the original document `decode_image` reads, and decoding fails. The test passes for that reason, not the one its doc comment gives ("pixel data does not hold three or four bytes per pixel"), which #46 made stale (inferred, read at 88b2a08).
- Colours in form XObjects, patterns and `sc`/`scn` stay in colour, so a "grayscale" output can still show colour (inferred).
- Operands split from their operator across two `/Contents` streams are lost when the first stream is rewritten (inferred, above). The compress-local `/Contents` helper decided on #138 covers indirect arrays, not this. Tracked for indirect arrays only: #138.
