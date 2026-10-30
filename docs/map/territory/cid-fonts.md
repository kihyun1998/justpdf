# CID fonts (reading Type0)

## What it is
The path that reads Type0 composite fonts and their descendant CIDFont: the encoding CMap (codes to CIDs and the writing mode), the `/W` and `/DW` widths, and `/CIDToGIDMap`. The CMap and the widths are read in core for every consumer; `/CIDToGIDMap` is still read by each consumer that needs it.

## Governing decisions
**None.** No decision record. The spec-over-MuPDF call for CMap decoding was the maintainer's on #303 (2026-10-08) and is written under the design model.

## Design model
Status: from reading the source and the tests, plus the maintainer's judgement on #303.

- **Codes become CIDs in core** (#303). `font::type0_encoding_cmap(font, resolve, decode)` reads a Type0 font's `/Encoding` into an `EncodingCMap`:
  - `Identity-H`/`-V` as an identity two-byte CMap (`EncodingCMap::identity`, `named_cmap`);
  - an embedded CMap stream (`embedded_cmap`): codespace ranges of 1–4 bytes, `cidrange`/`cidchar`, `notdefrange`/`notdefchar`, `/WMode`, and the CMap it uses — `/UseCMap` as an embedded stream (followed at most `MAX_USE_CMAP_DEPTH` = 8 deep, a reference already followed refused) or a name, else the `usecmap` operator's name. The used CMap's mappings come after the CMap's own, and its codespace is taken when the CMap has none (`EncodingCMap::use_cmap`).
  - A CMap with no codespace at all gets `<0000> <FFFF>`, as MuPDF does.
  - Within one CMap a mapping defined later replaces the codes it shares with an earlier one — a broad `cidrange` followed by single-code exceptions is common — as MuPDF's `add_range` does (`Ranges::insert` keeps disjoint segments).
  - A CID past 2³²−1 reads as 0 (`CidRange::cid_of`).
  - `/WMode` in the stream dictionary wins over a `/WMode … def` in the stream, as in MuPDF's `pdf_load_embedded_cmap`.
  - A named encoding other than Identity (#306), a missing one, or a stream that does not decode or parse falls back to Identity-H, the behaviour before #303.
  - The parser uses `content::parse_content_stream` as its PostScript tokenizer.
- **Strings are split and mapped as ISO 32000 §9.7.6.2–9.7.6.3 says** (`EncodingCMap::next_code`; read in ISO 32000-1:2008 on 2026-10-08, ISO 32000-2 assumed the same and not read, #303): the shortest whole code matching a codespace range, compared **byte by byte** (each byte between the corresponding bytes of the bounds, `Codespace::matches`); a valid code with no mapping takes its notdef mapping, else CID 0; bytes matching no range take the length of the best partial match (the shortest range length when the first byte begins none; else the range with the longest partial match, the shortest such range on a tie; `invalid_code_len`) and their notdef mapping, else CID 0.
  - **Following the spec over MuPDF is the maintainer's judgement** (2026-10-08, #303), made on this comparison: MuPDF (`pdf_decode_cmap`, `pdf_lookup_cmap` in `source/pdf/pdf-cmap.c`, call site in `pdf-op-run.c`, read raw 2026-10-08) compares a code to a range as one number, skips the glyph of a code with no mapping (no advance), does not parse notdef mappings, and takes one byte as code 0 for bytes matching no range. #303's triage brief said "shortest range length, CID 0, as MuPDF does", which matches neither.
  - One behaviour moved for Identity-H: an odd byte left at the end of a string used to be read as a one-byte code; it is now an invalid code (CID 0, its width), as the spec says (#303).
- **Consumers.** Text extraction (`show_string`), the raster renderer and the SVG renderer (`render_text_string`) read Type0 strings through the CMap and look up widths by CID. The raster renderer also selects the glyph by CID (`/CIDToGIDMap`, or a CID-keyed CFF's charset through `cff_cid_to_gid`); a CID past 65535 draws `.notdef` (GID 0) rather than a CID cut to 16 bits (`u16::try_from(cid)` in `render_text_string`). ToUnicode, and the code-32 test for word spacing, stay keyed by the **code**. Each consumer holds the CMap in its own resolved-font struct (`ResolvedFont::encoding` in core, `cid_encoding` in render); `FontInfo` does not carry it, to keep that public struct unchanged (#303). `FontEncoder` reads it too, to write codes at a whole-code length ([Font encodings](font-encodings.md)).
- **Writing mode is parsed and not used.** `EncodingCMap::wmode` has no caller outside `encoding_cmap.rs` (inferred, `git grep` 2026-10-30): a vertical font (`Identity-V`, `/WMode 1`) advances horizontally in text extraction and rendering. Vertical metrics (`/W2`, `/DW2`) are not read.
- **Widths are read in core** (#222), the same for text extraction, both renderers and `FontEncoder`. `font::descendant_font(dict, resolve)` takes the first `/DescendantFonts` element (the array and the element each resolved when indirect, a direct dictionary accepted); `resolve_font_entries` resolves `/W` (each element, and each value inside a `c [w1 w2 …]` list) and `/DW`; and `parse_font_info` on a `CIDFontType0`/`CIDFontType2` dictionary returns `FontWidths::CID` (`parse_cid_widths`): both `/W` forms, `/DW` 1000 when absent, and a non-number inside a list 0 wide so the widths after it keep their CIDs — what MuPDF's `pdf_array_get_int` gives (`source/pdf/pdf-font.c`, read raw 2026-10-08, #222).
  - A malformed `c_first c_last w` entry stops `/W` parsing at that point; the entries after it are lost (inferred, `break` in `parse_cid_widths`).
  - `FontWidths::get_width` returns the first `/W` entry that covers a CID, so an earlier entry wins over a later overlapping one (inferred).
  - Before #222 only text extraction read `/W` and `/DW`; it dropped a non-number from a list, shifting the rest by one CID, and read a descendant only when it was a reference, while both renderers advanced every glyph by 1000. A glyph with `/W` 2000 at 10 pt extracted 20.0 wide but rendered a 10-unit placeholder (measured 2026-10-06 on a synthetic Type0 page during #45's triage); `render_cid_widths.rs` now pins the 20-unit box.
  - A descendant reference that does not resolve leaves the Type0 font with its own default widths; before #222 the renderers failed to resolve the whole font and drew none of its text.
- `resolve_type0_descendant` forces `FontInfo::encoding` to `Identity` for a Type0 font and takes the descendant's descriptor when the Type0 dictionary has none. The encoding field then only drives the decoding fallback for a code ToUnicode does not map (inferred).
- **`/CIDToGIDMap` is read in three places**, each on its own: the raster renderer (`parse_cid_to_gid_map`, `parse_cid_gid_stream`), compress subsetting (`cid_font_glyph_ids`, [Font subsetting](font-subsetting.md)) and `FontEncoder` (`cid_to_gid_map`, indexing the stream two bytes per CID). Text extraction does not need it. The old note named two; `FontEncoder` was added by #178.
- **Input bounds.** The CMap stream and the font dictionaries come from the file. Codes are at most 4 bytes and `usecmap` chains at most 8 deep. Nothing bounds the number of codespace ranges, CID ranges or `/W` entries; `Ranges::insert` splices into a sorted vector, so a CMap of n mappings costs up to O(n²) (inferred).

## Code
- `justpdf-core/src/font/encoding_cmap.rs` — `EncodingCMap`, `EncodingCMap::identity`, `EncodingCMap::next_code`, `EncodingCMap::decode`, `EncodingCMap::is_whole_code`, `EncodingCMap::wmode`, `invalid_code_len`, `CMapCode`, `Codespace`, `CidRange`, `Ranges`, `Ranges::insert`, `type0_encoding_cmap`, `embedded_cmap`, `named_cmap`, `MAX_USE_CMAP_DEPTH`
- `justpdf-core/src/font/mod.rs` — `descendant_font`, `parse_cid_widths`, `resolve_font_entries`, `CIDWidthEntry`
- `justpdf-core/src/content/interpret.rs` — `resolve_type0_descendant`, `load_font`, `show_string`
- `justpdf-render/src/interpreter.rs` — `parse_cid_to_gid_map`, `parse_cid_gid_stream`, `render_text_string`
- `justpdf-core/tests/cid_cmap.rs` — `a_code_advances_by_the_width_of_the_cid_the_cmap_maps_it_to`, `mixed_one_and_two_byte_codes_split_by_the_codespace`, `identity_h_is_unchanged`
- `justpdf-core/tests/cid_widths.rs` — `both_w_forms_place_the_glyphs`, `an_indirect_width_inside_a_w_list_is_read`, `a_non_number_in_a_w_list_keeps_the_widths_after_it_in_place`, `a_direct_descendant_font_is_read`
- `justpdf-render/tests/render_cid_cmap.rs` — `a_raster_box_is_as_wide_as_the_mapped_cids_width`, `an_svg_placeholder_is_as_wide_as_the_mapped_cids_width`, `the_raster_glyph_comes_from_cid_to_gid_map_at_the_mapped_cid`
- `justpdf-render/tests/render_cid_widths.rs` — `a_raster_box_is_as_wide_as_w`, `raster_glyphs_advance_by_both_w_forms`, `an_svg_placeholder_is_as_wide_as_w`
- `scripts/make-cid-cff-fixture.py` — builds the CID-keyed CFF fixtures (`cid-test.cff`, `cid-test-half.cff`) that test CID → GID through a CFF charset ([CFF parser](cff.md))

Measured: the tests in `cid_cmap.rs`, `cid_widths.rs`, `render_cid_cmap.rs` and `render_cid_widths.rs` pass at 88b2a08 (2026-10-30).

## Reference behaviour
- MuPDF `source/pdf/pdf-cmap.c` (`pdf_decode_cmap`, `pdf_lookup_cmap`, `add_range`, `pdf_load_embedded_cmap`) and `pdf-op-run.c`, read raw 2026-10-08 (#303) — the comparison behind following the spec on code splitting, and the source of the later-wins, default-codespace and `/WMode` precedence rules.
- MuPDF `source/pdf/pdf-font.c` `pdf_array_get_int`, read raw 2026-10-08 (#222) — a non-number in a `/W` list is 0.
- ISO 32000-1:2008 §9.7.6.2–9.7.6.3, read 2026-10-08 (#303). ISO 32000-2 §9.7 not compared.

## Cross-cutting invariants
- [Font resolution](../invariant/font-resolution.md) — the CMap and widths are shared since #303 and #222; GID mapping is still read per consumer.

## Blast radius
- [Content interpreter](content-interpreter.md), [Text extraction](text-extraction.md), [Glyph rendering](glyph-rendering.md), [SVG renderer](svg-renderer.md) — all split strings with `next_code` and advance by CID widths; a change in either moves glyphs on every path at once, and a consumer that splits on its own would drift.
- [Font loading](font-loading.md) — `parse_font_info` fills CID widths; both rely on `resolve_font_entries` having resolved `/W` and `/DW`.
- [Font subsetting](font-subsetting.md) — reads `/CIDToGIDMap` on its own and rewrites `/W`; a different reading of the map here keeps or drops other glyphs.
- [Font encodings](font-encodings.md) — `FontEncoder` writes codes at the length `is_whole_code` accepts and reads `/CIDToGIDMap` on its own.
- [CJK font embedding](cjk-font-embedding.md) — this path reads the Identity-H Type0 fonts the writing side makes; a writer change must still decode here.
- [Redaction](redaction.md) — erases per glyph by the CID width this path gives; a width too small under-erases (ADR 0004).

## Known holes / open
- Predefined CMaps other than Identity (`UniJIS-UCS2-H`, `90ms-RKSJ-H`, …) fall back to Identity-H. Tracked: #306.
- A Type0 font without ToUnicode extracts raw bytes; there is no CID → Unicode step through the character collection. Tracked: #328.
- Vertical writing mode is parsed and not used: glyphs advance horizontally in extraction (Tracked: #313), are not grouped into vertical columns (Tracked: #323), and are drawn as a horizontal run (Tracked: #324).
- Word spacing (Tw) is added to any code 32, also a two-byte one. Tracked: #314.
- `/W2` and `/DW2` are not read (inferred).
- A malformed range entry in `/W` drops every entry after it (inferred).
