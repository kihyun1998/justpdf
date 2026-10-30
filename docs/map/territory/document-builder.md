# Document builder (creating a new PDF)

## What it is
Builds a PDF from nothing. `DocumentBuilder` gathers fonts, pages, images, metadata and encryption, builds the Pages, Catalog and Info (and `/Encrypt`) objects, and serializes them (inferred, `writer/document.rs — DocumentBuilder::build`). Its fonts are `Font` handles held in a `FontRegistry` ([CJK font embedding](cjk-font-embedding.md)) and written at `build`. `PageBuilder` assembles one page's content stream (text, paths, image placement) as operator text (inferred, `writer/page.rs`). Every PDF output of the input-format converters in `justpdf-formats`, and of OCR and ZUGFeRD in `justpdf-special`, goes through this path (inferred, `git grep -l 'DocumentBuilder\|PageBuilder' -- justpdf-formats justpdf-special`).

## Governing decisions
**None.** No ADR covers document creation. The decisions below were made on issues and are attributed there.

## Design model
- **Standard fonts.** A standard font is a Type1 dictionary without embedding, declared `/Encoding /WinAnsiEncoding` except for `Symbol` and `ZapfDingbats`, which keep their built-in encoding (inferred, `standard_font_dict`). `standard_font_dict` is used by the `FontRegistry` for `add_standard_font`, by the deprecated `PageBuilder::add_font`'s inline dictionaries, and by the annotation, form and signature appearance writers (inferred, `git grep -n standard_font_dict`). Until #34 none of them declared an encoding. `add_standard_font` returns a `Font`, the same one for the same name, since #173; before that it returned the resource name.
- **Text.** `PageBuilder::set_font(&Font, size)` writes `Tf` and declares the font in the page's resources. `show_text` encodes for the selected `Font`, as WinAnsi codes or 2-byte CIDs with fallback runs; the rules are in [CJK font embedding](cjk-font-embedding.md). After the deprecated `set_font_name`, or with no font selected, `show_text` writes `encode_winansi(text)` (inferred, `show_text`). Either way the bytes go through `write_string`: `(…) Tj` when they are printable ASCII, `<hex> Tj` otherwise (#29). In a standard font, a character with no code becomes `?` (a space in `ZapfDingbats`) and is reported by `missing_chars()` (inferred, `show_text` doc comment; measured on #34 and #173) — [Content text encoding](../invariant/content-text-encoding.md).
- `move_to(x, y)` writes `1 0 0 1 x y Tm`: it sets the text matrix and the text line matrix to a translation to (x, y), so each call is an absolute position, however many come before it in the same `BT` (measured: `move_to_places_each_run_at_its_own_point_in_one_text_block` reads the runs back at (72, 700) and (72, 650); #407). It also clears any earlier text-matrix change in that text object; `PageBuilder` writes no other text-matrix operator (inferred, `git grep` over `writer/page.rs`).
- `add_page` builds the page at once and panics when the page draws with a `Font` of another document, compared through the registry's document id (inferred, `add_page`, `FontRegistry::owns`). The embedded fonts are written by `build`, after every page, so their subsets cover every page (inferred, `build` calls `FontRegistry::write` first).
- `embed_truetype_font` (deprecated since #173) builds `Widths` and ToUnicode for codes 32–255 from the WinAnsi character of each code. Until #34 it took the code as Latin-1, so 0x80–0x9F (`€`, `—`, `“`) got width 0 and no ToUnicode entry (measured with Noto Sans, #34). It returns only the resource name, and its `IndirectRef` is reachable only through the test-only `font_ref`, so a page cannot use that font through the public API (inferred). Since #173 a page uses an embedded font through `add_font` and `set_font(&Font)` instead, which resolved #65; `embed_truetype_font` stays, deprecated, with unchanged behaviour.
- Info values such as `set_title` are stored as `as_bytes()`, UTF-8 without a BOM (inferred, `set_title`) — [Text string encoding](../invariant/text-string-encoding.md).
- `set_font`, `draw_image` and `draw_inline_image` write names with `name_syntax` (#29). Coordinate, colour and font-size numbers are written with `Number`: integral values within the 32-bit range as integers, NaN and ±Inf as finite numbers (inferred, `writer/page.rs`; #90, the generator reals of [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md)).
- `draw_inline_image` writes only `BI … ID … EI`, with no `cm` (inferred). The image maps onto the unit square, so the caller has to write the `cm` first. The data goes in as it is, so if it contains whitespace + `EI` + whitespace or a delimiter, a reader ends the image there. It is meant for small images (its doc comment).
- A raw RGB raster goes in through `embed_rgb` (length check, Flate, an 8-bit DeviceRGB XObject) and is drawn with `draw_image` (inferred, `embed_rgb`, `add_rgb_image`). The CBZ, SVG and OCR converters take this path. **Maintainer decision (2026-09-25, #88)**: large rasters go in as XObjects instead of inline images. Alternatives offered: `/AHx` hex encoding only when the pattern occurs, or a length-based parser (other readers could still misread it). `embed_png` uses the same XObject creation (`add_rgb_image`).
- XMP values go into `set_xmp_metadata`'s template without XML escaping (inferred, `set_xmp_metadata`).
- **`/ID`.** When encrypting, the file ID is 16 bytes from `random_file_id`, and the two `/ID` elements are the same value, as ISO 32000-1 §14.4 asks for a file's first write (inferred, `build`) — [Object encryption](object-encryption.md). Without encryption, `build` calls the public `serialize_pdf`, which writes no `/ID` (inferred).
- **Input size**: a caller hands in pages, images and text. `embed_rgb` checks that the pixel buffer is `width × height × 3` bytes (inferred). Nothing bounds the page count, the content length or an image's dimensions.

## Code
- `justpdf-core/src/writer/document.rs` — `DocumentBuilder`, `standard_font_dict`, `add_standard_font`, `add_font`, `add_font_with`, `missing_chars`, `add_page`, `embed_truetype_font`, `font_ref`, `set_title`, `set_author`, `set_subject`, `set_producer`, `set_creator`, `set_xmp_metadata`, `set_encryption`, `build`, `save`, `embed_jpeg`, `embed_png`, `embed_rgb`, `add_rgb_image`, `generate_tounicode_cmap`
- `justpdf-core/src/writer/page.rs` — `PageBuilder`, `move_to`, `show_text`, `set_font`, `set_font_name`, `font_entries`, `add_font`, `add_font_ref`, `add_image`, `draw_image`, `draw_inline_image`, `build`
- `justpdf-core/src/writer/encode.rs` — `make_stream`, `encode_flate`, `encode_flate_best`

## Reference behaviour
**None.** No output of the builder has been compared with another writer's. ISO 32000-1 §14.4 (`/ID` on first write) was read on #31 (2026-09-24); see [Object encryption](object-encryption.md).

## Cross-cutting invariants
- [Content text encoding](../invariant/content-text-encoding.md) — `show_text` is the representative site.
- [Text string encoding](../invariant/text-string-encoding.md) — writing Info values.
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md) — `PageBuilder` writes names and strings through the shared functions and numbers through `Number`.

## Blast radius
- [File serialization](file-serialization.md), [Object encryption](object-encryption.md) — `build` ends there. The builder writes every object at generation 0 and assumes the serializer adds `/Encrypt` and `/ID` on the encrypted path only.
- [Format document](format-document.md), each input format, [OCR](ocr.md) — every `to_pdf` uses this API. They assume the meaning of `show_text`, `move_to` and `draw_image` (what `show_text` does with a character the font lacks, that `move_to` places text at an absolute point). CBZ, SVG and OCR embed pixels with `embed_rgb` and draw them with `draw_image` (`git grep`, 2026-10-30); nothing outside core calls `draw_inline_image`.
- [CJK font embedding](cjk-font-embedding.md), [ToUnicode](tounicode.md) — share the assumption that `build` writes the registered fonts after every page, so subsets cover every page.
- [Annotation appearance](annotation-appearance.md), [Form appearance](form-appearance.md), [Signature appearance](signature-appearance.md) — use `standard_font_dict`. A change to the standard font dictionary (its encoding) changes how their text is decoded.
- [Facade](facade.md) — re-exports `DocumentBuilder` and `PageBuilder`; a signature change here is a facade API change.

## Known holes / open
- There is no UTF-16BE text string encoder: Info values are written as BOM-less UTF-8. Tracked: #33
- Without encryption no `/ID` is written. `/ID` is required in PDF 2.0 (Arlington model, read on #77, 2026-10-10), and the maintainer decided on 2026-10-10 that every complete file write gets one (#77). Tracked: #77
- XMP values are not XML-escaped, so a title holding `<` or `&` produces malformed XMP (inferred, `set_xmp_metadata`).
