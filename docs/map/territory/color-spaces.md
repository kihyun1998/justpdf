# Color spaces

## What it is
The PDF color space model (Device*, CalGray/CalRGB, Lab, Indexed, Separation, DeviceN, ICCBased) and conversion of color values to RGB, an ICC profile parser and sRGB conversion, and OutputIntent and overprint parsers.

## Governing decisions
**None.**

## Design model
- `from_array` always builds ICCBased with `num_components: 3`, `profile: None` (comment: "actual value requires reading the stream").
- `Color::to_rgb` returns black for Lab, Indexed, Separation, DeviceN and ICCBased without a profile. CalGray/CalRGB are treated as Device.
- **ICC, OutputIntent and overprint are unreachable from product paths**: the only callers of `parse_icc_profile`, `read_output_intents` and `parse_overprint` are tests, and `icc_to_srgb` is reached only through `to_rgb` with a profile set, which is never set.
- Besides this module (`cmyk_to_rgb`), the renderer, SVG, shading and compression each keep their own copy of CMYK→RGB conversion — [Image pixel layout](../invariant/image-pixel-layout.md).

## Code
- `justpdf-core/src/color/mod.rs` — `ColorSpace`, `from_pdf_object`, `from_array`, `num_components`, `Color`, `to_rgb`, `cmyk_to_rgb`, `rgb_to_cmyk`, `OutputIntent`, `read_output_intents`, `parse_overprint`
- `justpdf-core/src/color/icc.rs` — `IccProfile`, `parse_icc_profile`, `icc_to_srgb`, `RenderingIntent`

## Reference behaviour
**None.** The code cites ICC.1:2004 / ISO 15076-1, OutputIntent §14.11.5, Overprint §8.6.7 (not a comparison record). Clause to compare against: ISO 32000-2 §8.6.

## Cross-cutting invariants
- [Image pixel layout](../invariant/image-pixel-layout.md) — the original of the color conversion copies.

## Blast radius
- [Render interpreter](render-interpreter.md) — `cs`/`CS` use a local `cs_from_name` (names only) and bypass this module's array color spaces. Extending color space support starts there.
- [SVG renderer](svg-renderer.md), [Render shading](render-shading.md) — each has its own name-only copy.
- [Image decoding](image-decoding.md) — reads an image's color space by name only.
- [Compress images](compress-images.md) — skips CMYK and converts on its own with `to_rgb_pixels`.

## Known holes / open
- `test_indexed` checks only the component count, base and hival. No code looks up an actual color in an Indexed palette.
- Rendering Intent is only parsed and applied nowhere.
