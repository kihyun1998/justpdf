# Color spaces

## What it is
The PDF colour space model (`ColorSpace`: Device*, CalGray/CalRGB, Lab, Indexed, Separation, DeviceN, ICCBased, Unknown), reading a colour space through the document, the colour set by `cs`/`CS`, and conversion of colour values to RGB. It also holds an ICC profile parser with sRGB conversion, and OutputIntent and overprint parsers.

## Governing decisions
**None.** The colour-space recursion bound was decided on #414 (maintainer, 2026-10-30: MuPDF's ancestor-chain rule with depth 256, a refused level becoming `Unknown`; shown beside returning a `Result`), not in a record.

## Design model
Rules read from the code (*inferred* unless marked).
- **`ColorSpace::resolve(obj, doc)`** reads a colour space through references (#46): the space itself, the references inside its array, an Indexed lookup given as a string or a stream (decoded; a failure gives an empty lookup), an ICCBased stream's `/N` (clamped to 1..4, default 3) and `/Alternate`, and the alternate space and tint transform of Separation and DeviceN (`PdfFunction::resolve`, so a sampled or filtered transform converts, #47). A reference that does not resolve gives `Unknown("unresolved")`.
- `from_pdf_object` (no document) builds ICCBased with `num_components: 3` and no alternate, reads an Indexed lookup only from a string, and parses only a direct tint transform. It is reached only from `resolve`'s fallback for names and other arrays, and from tests.
- `Pattern` is not a `ColorSpace` variant: the name reads as `Unknown(b"Pattern")`.
- **`initial_color`** gives the colour `cs`/`CS` sets (ISO 32000-2 §8.6.8): 0 in each component, ICCBased too whatever its `/N`; DeviceCMYK `0 0 0 1`; Indexed index 0; Separation and DeviceN tint 1 in each colorant (#231).
- **`Color::to_rgb`** converts Indexed (index rounded and clamped to `hival`, the lookup entry read in the base space; a short lookup gives 0 in each base component), Separation and DeviceN (through the tint transform into the alternate; without one, the tint shows as gray `1 − t`), and ICCBased without a profile through `substitute`: its `/Alternate`, else DeviceGray/RGB/CMYK for `/N` 1/3/4, with no ICC transform. Lab and Unknown return black. CalGray and CalRGB are treated as their device spaces.
- **ICC, OutputIntent and overprint are unreachable from product paths:** the only callers of `parse_icc_profile`, `read_output_intents` and `parse_overprint` are tests, and `icc_to_srgb` is reached only through `to_rgb` with a profile set, which nothing sets (`rg -n 'profile: Some' -g '*.rs'` matches only the `to_rgb` arm, 2026-10-30). Rendering intent is parsed and applied nowhere.
- **CMYK → RGB:** `cmyk_to_rgb` is the formula `Color::to_rgb` and `icc_to_srgb` use, and the renderers reach it through `to_rgb`. Two copies remain outside this module: the 8-bit fast path of `DecodedImage::to_rgb8` (the same formula, truncated instead of rounded) and `compress.rs — cmyk_to_gray_from_operands` (CMYK to gray) — [Image pixel layout](../invariant/image-pixel-layout.md).
- **Input bound:** `resolve` follows an Indexed base, an ICCBased `/Alternate` and a Separation/DeviceN alternate through `resolve_within`, carrying the object numbers of the colour spaces it is inside (0 for a direct one). One that is its own ancestor — an ICCBased also counted by its ICC stream, so a stream reached again inside its own `/Alternate` is refused — or one past `MAX_COLOR_SPACE_DEPTH` (256) entries, direct arrays included, is `Unknown(b"recursive")` at that level, as an unresolved reference is `Unknown(b"unresolved")`. An ICCBased level adds two entries (its array and its stream), so a chain of ICC alternates stops after 128 (inferred). Pinned by the `color::tests` recursion tests (measured 2026-10-30). Depth 256 holds on a 1 MiB thread in a debug build, resolve, `to_rgb`, clone, compare and drop of a 300-level Indexed and Separation chain all completing (measured 2026-10-30, throwaway probe). No colour space has two colour-space children, so the ancestor chain and a visited set behave alike within one call (dropping the `pop` left the tests green, measured).

## Code
- `justpdf-core/src/color/mod.rs` — `ColorSpace`, `ColorSpace::resolve`, `ColorSpace::resolve_within`, `ColorSpace::resolve_one`, `MAX_COLOR_SPACE_DEPTH`, `ColorSpace::substitute`, `ColorSpace::initial_color`, `ColorSpace::num_components`, `ColorSpace::from_pdf_object`, `ColorSpace::from_name`, `ColorSpace::from_array`, `ColorSpace::parse_indexed`, `ColorSpace::parse_separation`, `ColorSpace::parse_device_n`, `Color`, `Color::to_rgb`, `cmyk_to_rgb`, `rgb_to_cmyk`, `OutputIntent`, `read_output_intents`, `OverprintState`, `parse_overprint`
- `justpdf-core/src/color/icc.rs` — `IccProfile`, `parse_icc_profile`, `icc_to_srgb`, `RenderingIntent`

## Reference behaviour
- Colour-space recursion (read raw for #414): MuPDF `pdf_load_colorspace_imp` (`source/pdf/pdf-colorspace.c` @ `b487eef`) checks the ancestor chain with `pdf_cycle` (`source/pdf/pdf-object.c`, `MAX_CYCLE_STACK_DEPTH` 256), counting indirect objects only, and throws "recursive colorspace"; Indexed and DeviceN rethrow, so the whole colour space fails, and only an ICC `/Alternate` swallows it. It does not count the ICC stream. justpdf takes the ancestor rule and the 256 from it; returning `Unknown` at the refused level, counting direct arrays and counting the ICC stream are #414's choices. pdf.js (`src/core/colorspace_utils.js` @ `22f5df1`) has no cycle check and relies on the engine's stack limit.
- The code cites ICC.1:2004 / ISO 15076-1, OutputIntent §14.11.5 and Overprint §8.6.7; that is not a comparison record. Image conversion through these spaces was compared with MuPDF under [Image decoding](image-decoding.md) (#46). Not compared: ISO 32000-2 §8.6.

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
- Lab draws black, and CalGray/CalRGB ignore their calibration (above).
- ICC profiles, OutputIntents, overprint and rendering intent are parsed but not used by any product path (above).
- Indexed lookups are tested through `DecodedImage::to_rgb8` (`justpdf-core/tests/image_rgb.rs`); `test_indexed` checks only the parsed fields.
