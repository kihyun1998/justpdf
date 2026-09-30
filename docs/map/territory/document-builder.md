# Document builder (creating a new PDF)

## What it is
Builds a PDF from nothing. `DocumentBuilder` gathers fonts, pages, images, metadata and encryption, builds Pages/Catalog/Info(/Encrypt) and serializes them. `PageBuilder` assembles one page's content stream (text, paths, image placement) as operator text. Every PDF output of the formats and special crates goes through this path.

## Governing decisions
**None.**

## Design model
- A standard font is a Type1 dictionary with neither `/Encoding` nor embedding (`add_standard_font`).
- `show_text` writes the UTF-8 bytes of the Rust string with `write_string` — `(…) Tj` if they are printable ASCII, otherwise the same bytes as `<hex> Tj` (#29) — [Content text encoding](../invariant/content-text-encoding.md).
- Info values such as `set_title` are stored as `as_bytes()` (UTF-8 without a BOM) — [Text string encoding](../invariant/text-string-encoding.md).
- `set_font`, `draw_image` and `draw_inline_image` write names with `name_syntax` (#29). Coordinate, color and font-size reals are written with `Number` — integral values as integers, NaN and inf as finite numbers (#90, the generator reals of [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md)).
- `draw_inline_image` writes only `BI … ID … EI` and no `cm`. The image maps onto the unit square, so the caller has to write the `cm` first. The data goes in as it is, so if it contains whitespace + `EI` + whitespace/delimiter, a reader ends the image there — it is meant for small images.
- A raw RGB raster goes in through `embed_rgb` (length check, Flate, DeviceRGB 8-bit XObject) and is drawn with `draw_image`. CBZ, SVG and OCR take this path. **Maintainer decision (2026-09-25, #88)**: large rasters as XObjects instead of inline. Alternatives offered: `/AHx` hex encoding only when the pattern occurs, a length-based parser (other readers could still misread it). `embed_png` also uses the same XObject creation (`add_rgb_image`).
- XMP values go in without XML escaping (`set_xmp_metadata`).
- `embed_truetype_font` returns only the resource name. The font's `IndirectRef` can be obtained only through `font_ref`, which exists only in test builds (`#[cfg(test)]`), and `PageBuilder` puts an embedded font into the resources only through `add_font_ref` (needs the ref) (`add_font` makes a standard Type1 inline dictionary). So with the public API alone an embedded TrueType font cannot be used on a page.
- When encrypting, the file ID is 16 bytes made by `random_file_id` and the two `/ID` elements are the same — [Object encryption](object-encryption.md). Without encryption no `/ID` is written.

## Code
- `justpdf-core/src/writer/document.rs` — `DocumentBuilder`, `add_standard_font`, `embed_truetype_font`, `font_ref`, `set_title`, `set_xmp_metadata`, `set_encryption`, `build`, `save`, `embed_jpeg`, `embed_png`, `embed_rgb`, `generate_tounicode_cmap`
- `justpdf-core/src/writer/page.rs` — `PageBuilder`, `show_text`, `set_font`, `draw_image`, `draw_inline_image`
- `justpdf-core/src/writer/encode.rs` — `make_stream`, `encode_flate`, `encode_flate_best`

## Reference behaviour
**None.**

## Cross-cutting invariants
- [Content text encoding](../invariant/content-text-encoding.md) — `show_text` is the representative site.
- [Text string encoding](../invariant/text-string-encoding.md) — writing Info values.
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md) — `PageBuilder` assembles operator text (names and strings through shared functions, reals by hand).

## Blast radius
- [File serialization](file-serialization.md), [Object encryption](object-encryption.md) — the end of `build`.
- [Format document](format-document.md) and each input format, [OCR](ocr.md) — every `to_pdf` uses this API. If the signature or meaning of `show_text` or `draw_inline_image` changes, check them all.
- [CJK font embedding](cjk-font-embedding.md), [ToUnicode](tounicode.md) — the path that writes embedded fonts.
- [Facade](facade.md) — re-exports `DocumentBuilder`/`PageBuilder`.

## Known holes / open
- The public API cannot connect an embedded TrueType font to a page (Design model above). Tracked: #65.
- There is no path to write non-ASCII text correctly: standard fonts are WinAnsi-family, and there is no UTF-16BE text string encoder either.
- Without encryption no `/ID` is written. ISO 32000-1 §14.4 says "optional but should be used"; whether it is required in PDF 2.0 was not checked.
- Tracked: #33 (text string encoding), #34 (non-ASCII content text), #43 (inline image missing `cm`), #77 (no `/ID` without encryption)
