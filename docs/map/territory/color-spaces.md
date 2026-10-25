# Color spaces

## What it is
The PDF color space model (Device*, CalGray/CalRGB, Lab, Indexed, Separation, DeviceN, ICCBased) and conversion of color values to RGB, an ICC profile parser and sRGB conversion, and OutputIntent and overprint parsers.

## Governing decisions
**None.**

## Design model
- **`ColorSpace::resolve(obj, doc)`** reads a colour space through references (#46): an Indexed lookup given as a string or a stream (decoded), an ICCBased stream's `/N` (clamped to 1..4) and `/Alternate`, and the alternate space and tint transform (`justpdf_core::function::PdfFunction`) of Separation and DeviceN. `from_pdf_object` (no document) still builds ICCBased with `num_components: 3` and no alternate, and parses only a direct tint transform.
- **`Color::to_rgb`** converts Indexed (index rounded and clamped to `hival`, the lookup entry read in the base space), Separation and DeviceN (through the tint transform into the alternate; without one, the tint shows as gray `1 − t`), and ICCBased without a profile through `substitute`: its `/Alternate`, else DeviceGray/RGB/CMYK for `/N` 1/3/4 (no ICC transform, #46 brief). It returns black for Lab. CalGray/CalRGB are treated as Device. `resolve` reads a tint transform with `PdfFunction::resolve`, so a sampled or filtered one converts (#47); one that does not parse leaves the tint shown as gray.
- **ICC, OutputIntent and overprint are unreachable from product paths**: the only callers of `parse_icc_profile`, `read_output_intents` and `parse_overprint` are tests, and `icc_to_srgb` is reached only through `to_rgb` with a profile set, which is never set.
- Besides this module (`cmyk_to_rgb`), the renderer, SVG, shading and compression each keep their own copy of CMYK→RGB conversion — [Image pixel layout](../invariant/image-pixel-layout.md).

## Code
- `justpdf-core/src/color/mod.rs` — `ColorSpace`, `resolve`, `substitute`, `initial_color`, `from_pdf_object`, `from_array`, `num_components`, `Color`, `to_rgb`, `cmyk_to_rgb`, `rgb_to_cmyk`, `OutputIntent`, `read_output_intents`, `parse_overprint`
- `justpdf-core/src/color/icc.rs` — `IccProfile`, `parse_icc_profile`, `icc_to_srgb`, `RenderingIntent`

## Reference behaviour
**None.** The code cites ICC.1:2004 / ISO 15076-1, OutputIntent §14.11.5, Overprint §8.6.7 (not a comparison record). Clause to compare against: ISO 32000-2 §8.6.

## Cross-cutting invariants
- [Image pixel layout](../invariant/image-pixel-layout.md) — the original of the color conversion copies.

## Blast radius
- [Render interpreter](render-interpreter.md), [SVG renderer](svg-renderer.md) — `cs`/`CS` resolve resource colour spaces with `resolve` and set `initial_color` (#231).
- [Render shading](render-shading.md) — converts shading output through `to_rgb` in the resolved space (#231).
- [Image decoding](image-decoding.md) — resolves an image's colour space with `resolve` and converts samples with `to_rgb`.
- [Compress images](compress-images.md) — skips CMYK; converts through `DecodedImage::to_rgb8`.

## Known holes / open
- Indexed lookups are tested through `DecodedImage::to_rgb8` (`justpdf-core/tests/image_rgb.rs`); `test_indexed` still checks only the parsed fields.
- Rendering Intent is only parsed and applied nowhere.
