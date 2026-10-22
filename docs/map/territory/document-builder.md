# Document builder (creating a new PDF)

## What it is
Builds a PDF from nothing. `DocumentBuilder` gathers fonts, pages, images, metadata and encryption, builds Pages/Catalog/Info(/Encrypt) and serializes them. Its fonts are `Font` handles held in a `FontRegistry` ([CJK font embedding](cjk-font-embedding.md)), written at `build`. `PageBuilder` assembles one page's content stream (text, paths, image placement) as operator text. Every PDF output of the formats and special crates goes through this path.

## Governing decisions
**None.**

## Design model
- A standard font is a Type1 dictionary without embedding, declared `/Encoding /WinAnsiEncoding` except `Symbol` and `ZapfDingbats` (`standard_font_dict`, used by `add_standard_font` and the deprecated `PageBuilder::add_font`'s inline dictionaries; until #34 neither declared an encoding). `add_standard_font` returns a `Font` (the same one for the same name) since #173; it returned the resource name.
- `PageBuilder::set_font(&Font, size)` writes `Tf` and declares the font in the page's resources; `show_text` encodes for the selected `Font` (WinAnsi or 2-byte CIDs, with fallback runs) — the rules are in [CJK font embedding](cjk-font-embedding.md). After the deprecated `set_font_name`, or with no font selected, `show_text` writes `encode_winansi(text)`. Either way the bytes go through `write_string` — `(…) Tj` if printable ASCII, otherwise `<hex> Tj` (#29). A character outside WinAnsi in a standard font becomes `?` and is reported by `missing_chars()` (measured, #34, #173) — [Content text encoding](../invariant/content-text-encoding.md).
- `add_page` builds the page at once and panics when the page draws with a `Font` of another document (the registry's document id). The embedded fonts are written by `build`, after every page, so their subsets cover every page.
- `embed_truetype_font` (deprecated since #173) builds `Widths` and ToUnicode for codes 32–255 from the WinAnsi character of each code; until #34 it took the code as Latin-1, so 0x80–0x9F (`€`, `—`, `“`) got width 0 and no ToUnicode entry (measured with Noto Sans).
- Info values such as `set_title` are stored as `as_bytes()` (UTF-8 without a BOM) — [Text string encoding](../invariant/text-string-encoding.md).
- `set_font`, `draw_image` and `draw_inline_image` write names with `name_syntax` (#29). Coordinate, color and font-size reals are written with `Number` — integral values as integers, NaN and inf as finite numbers (#90, the generator reals of [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md)).
- `draw_inline_image` writes only `BI … ID … EI` and no `cm`. The image maps onto the unit square, so the caller has to write the `cm` first. The data goes in as it is, so if it contains whitespace + `EI` + whitespace/delimiter, a reader ends the image there — it is meant for small images.
- A raw RGB raster goes in through `embed_rgb` (length check, Flate, DeviceRGB 8-bit XObject) and is drawn with `draw_image`. CBZ, SVG and OCR take this path. **Maintainer decision (2026-09-25, #88)**: large rasters as XObjects instead of inline. Alternatives offered: `/AHx` hex encoding only when the pattern occurs, a length-based parser (other readers could still misread it). `embed_png` also uses the same XObject creation (`add_rgb_image`).
- XMP values go in without XML escaping (`set_xmp_metadata`).
- `embed_truetype_font` returns only the resource name, and its `IndirectRef` is reachable only through the test-only `font_ref`, so a page cannot use that font through the public API. Since #173 a page uses an embedded font through `add_font` + `set_font(&Font)` instead, which resolves #65; `embed_truetype_font` stays, deprecated, with unchanged behaviour.
- When encrypting, the file ID is 16 bytes made by `random_file_id` and the two `/ID` elements are the same — [Object encryption](object-encryption.md). Without encryption no `/ID` is written.

## Code
- `justpdf-core/src/writer/document.rs` — `DocumentBuilder`, `standard_font_dict`, `add_standard_font`, `add_font`, `add_font_with`, `missing_chars`, `add_page`, `embed_truetype_font`, `font_ref`, `set_title`, `set_xmp_metadata`, `set_encryption`, `build`, `save`, `embed_jpeg`, `embed_png`, `embed_rgb`, `generate_tounicode_cmap`
- `justpdf-core/src/writer/page.rs` — `PageBuilder`, `show_text`, `set_font`, `set_font_name`, `font_entries`, `draw_image`, `draw_inline_image`
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
- There is no UTF-16BE text string encoder (#33).
- Without encryption no `/ID` is written. ISO 32000-1 §14.4 says "optional but should be used"; whether it is required in PDF 2.0 was not checked.
- Tracked: #33 (text string encoding), #77 (no `/ID` without encryption)
