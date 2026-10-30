# Color spaces

## What it is
The PDF colour space model (`ColorSpace`: Device*, CalGray/CalRGB, Lab, Indexed, Separation, DeviceN, ICCBased, Unknown), reading a colour space through the document, the colour set by `cs`/`CS`, and conversion of colour values to RGB. It also holds an ICC profile parser with sRGB conversion, and OutputIntent and overprint parsers.

## Governing decisions
**None.**

## Design model
Rules read from the code (*inferred* unless marked).
- **`ColorSpace::resolve(obj, doc)`** reads a colour space through references (#46): the space itself, the references inside its array, an Indexed lookup given as a string or a stream (decoded; a failure gives an empty lookup), an ICCBased stream's `/N` (clamped to 1..4, default 3) and `/Alternate`, and the alternate space and tint transform of Separation and DeviceN (`PdfFunction::resolve`, so a sampled or filtered transform converts, #47). A reference that does not resolve gives `Unknown("unresolved")`.
- `from_pdf_object` (no document) builds ICCBased with `num_components: 3` and no alternate, reads an Indexed lookup only from a string, and parses only a direct tint transform. It is reached only from `resolve`'s fallback for names and other arrays, and from tests.
- `Pattern` is not a `ColorSpace` variant: the name reads as `Unknown(b"Pattern")`.
- **`initial_color`** gives the colour `cs`/`CS` sets (ISO 32000-2 §8.6.8): 0 in each component, ICCBased too whatever its `/N`; DeviceCMYK `0 0 0 1`; Indexed index 0; Separation and DeviceN tint 1 in each colorant (#231).
- **`Color::to_rgb`** converts Indexed (index rounded and clamped to `hival`, the lookup entry read in the base space; a short lookup gives 0 in each base component), Separation and DeviceN (through the tint transform into the alternate; without one, the tint shows as gray `1 − t`), and ICCBased without a profile through `substitute`: its `/Alternate`, else DeviceGray/RGB/CMYK for `/N` 1/3/4, with no ICC transform. Lab and Unknown return black. CalGray and CalRGB are treated as their device spaces.
- **ICC, OutputIntent and overprint are unreachable from product paths:** the only callers of `parse_icc_profile`, `read_output_intents` and `parse_overprint` are tests, and `icc_to_srgb` is reached only through `to_rgb` with a profile set, which nothing sets (`rg -n 'profile: Some' -g '*.rs'` matches only the `to_rgb` arm, 2026-10-30). Rendering intent is parsed and applied nowhere.
- **CMYK → RGB:** `cmyk_to_rgb` is the formula `Color::to_rgb` and `icc_to_srgb` use, and the renderers reach it through `to_rgb`. Two copies remain outside this module: the 8-bit fast path of `DecodedImage::to_rgb8` (the same formula, truncated instead of rounded) and `compress.rs — cmyk_to_gray_from_operands` (CMYK to gray) — [Image pixel layout](../invariant/image-pixel-layout.md).
- **Input bound:** `resolve` recurses into an Indexed base, an ICCBased `/Alternate` and a Separation/DeviceN alternate with no depth limit and no cycle check. *Measured* 2026-10-30, release `justpdf render` on a page drawing a 1 × 1 image whose `/ColorSpace` is `6 0 R` with object 6 `[/Indexed 6 0 R 0 <00>]`: the process aborts with a stack overflow (exit 134). An Indexed lookup and a DeviceN name list are bounded only by their decoded size; `hival` is clamped at 0 from below and not checked against the lookup length.

## Code
- `justpdf-core/src/color/mod.rs` — `ColorSpace`, `ColorSpace::resolve`, `ColorSpace::substitute`, `ColorSpace::initial_color`, `ColorSpace::num_components`, `ColorSpace::from_pdf_object`, `ColorSpace::from_name`, `ColorSpace::from_array`, `ColorSpace::parse_indexed`, `ColorSpace::parse_separation`, `ColorSpace::parse_device_n`, `Color`, `Color::to_rgb`, `cmyk_to_rgb`, `rgb_to_cmyk`, `OutputIntent`, `read_output_intents`, `OverprintState`, `parse_overprint`
- `justpdf-core/src/color/icc.rs` — `IccProfile`, `parse_icc_profile`, `icc_to_srgb`, `RenderingIntent`

## Reference behaviour
**None.** The code cites ICC.1:2004 / ISO 15076-1, OutputIntent §14.11.5 and Overprint §8.6.7; that is not a comparison record. Image conversion through these spaces was compared with MuPDF under [Image decoding](image-decoding.md) (#46). Not compared: ISO 32000-2 §8.6.

## Cross-cutting invariants
- [Image pixel layout](../invariant/image-pixel-layout.md) — `cmyk_to_rgb` is the original of the CMYK conversion copies.

## Blast radius
- [Render interpreter](render-interpreter.md), [SVG renderer](svg-renderer.md) — `cs`/`CS` resolve resource colour spaces with `resolve` and set `initial_color`, and every fill and stroke converts through `to_rgb` (#231). A change to a space's component count or default changes every painted colour there.
- [Render shading](render-shading.md) — converts shading output through `to_rgb` in the resolved space (#231).
- [Render transparency](render-transparency.md) — resolves a transparency group's `/CS` with `resolve`.
- [Image decoding](image-decoding.md) — resolves an image's colour space with `resolve`, takes its component count from `num_components`, and converts samples through `substitute` and `to_rgb`.
- [Compress images](compress-images.md), [Compress grayscale](compress-grayscale.md) — convert through `DecodedImage::to_rgb8`, and grayscale keeps its own CMYK-to-gray formula.
- [PDF functions](pdf-functions.md) — tint transforms are `PdfFunction`s; a function that does not parse leaves the tint shown as gray.

## Known holes / open
- A colour space that refers to itself, directly or through an alternate or base, overflows the stack (measured above). Tracked: #414.; #122's depth limit lists tree walkers and parsers, not this resolver.
- Lab draws black, and CalGray/CalRGB ignore their calibration (above).
- ICC profiles, OutputIntents, overprint and rendering intent are parsed but not used by any product path (above).
- Indexed lookups are tested through `DecodedImage::to_rgb8` (`justpdf-core/tests/image_rgb.rs`); `test_indexed` checks only the parsed fields.
