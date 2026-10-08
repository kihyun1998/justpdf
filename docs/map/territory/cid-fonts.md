# CID fonts (reading Type0)

## What it is
The path that reads Type0 composite fonts and their descendant CIDFont: the encoding CMap (codes to CIDs), `/W` and `/DW` widths, `/CIDToGIDMap`. The CMap and the widths are read in core; `/CIDToGIDMap` is still implemented by each consumer that needs it.

## Governing decisions
**None.** No decision record; the spec-over-MuPDF call for CMap decoding (#303) is recorded under the design model.

## Design model
- **Codes become CIDs in core** (#303): `font::type0_encoding_cmap(font, resolve, decode)` reads a Type0 font's `/Encoding` into an `EncodingCMap` — `Identity-H`/`-V` as an identity two-byte CMap, or an embedded CMap stream (codespace ranges of 1–4 bytes, `cidrange`/`cidchar`, `notdefrange`/`notdefchar`, `/WMode`, and the CMap it uses: `/UseCMap` as an embedded stream (followed at most 8 deep, a reference already followed refused) or a name, or the `usecmap` operator's name; the used CMap's mappings come after the CMap's own, and its codespace is taken when the CMap has none). A CMap with no codespace at all gets `<0000> <FFFF>`, as MuPDF does. Within one CMap, a mapping defined later replaces the codes it shares with an earlier one — a broad `cidrange` followed by single-code exceptions is common — as MuPDF's `add_range` does (`Ranges::insert` keeps disjoint segments). A CID past 2³²−1 reads as 0. `/WMode` in the stream dictionary wins over a `/WMode … def` in the stream, as in MuPDF's `pdf_load_embedded_cmap`. A named encoding other than Identity (#306), a missing one, or a stream that does not decode or parse falls back to Identity-H — the behaviour before #303. The parser reuses `content::parse_content_stream` as its PostScript tokenizer.
- Strings are split and mapped as ISO 32000 §9.7.6.2–9.7.6.3 says (`EncodingCMap::next_code`; read in ISO 32000-1:2008, 2026-10-08 — ISO 32000-2 assumed the same, not read): the shortest whole code matching a codespace range, **compared byte by byte** (each byte between the corresponding bytes of the bounds); a valid code with no mapping takes its notdef mapping, else CID 0; bytes matching no range take the length of the best partial match (the shortest range length when the first byte begins none; else the range with the longest partial match, the shortest such range on a tie) and their notdef mapping, else CID 0. **Following the spec over MuPDF is the maintainer's judgement** (2026-10-08, #303), made on this comparison: MuPDF (`pdf_decode_cmap`, `pdf_lookup_cmap` in `source/pdf/pdf-cmap.c`, call site in `pdf-op-run.c`, read raw 2026-10-08) compares a code to a range as one number, **skips** the glyph of a code with no mapping (no advance), does not parse notdef mappings, and takes one byte as code 0 for bytes matching no range. #303's triage brief said "shortest range length, CID 0, as MuPDF does", which matches neither.
- Text extraction (`show_string`), the raster renderer and the SVG renderer (`render_text_string`) read Type0 strings through it and look up widths by CID; the raster renderer also selects the glyph by CID (`/CIDToGIDMap`, a CID-keyed CFF's charset); a CID past 65535 draws `.notdef` (GID 0) rather than a CID cut to 16 bits. ToUnicode, and the code-32 test for word spacing, stay keyed by the **code**. Each consumer holds the CMap in its own resolved-font struct (`ResolvedFont::encoding` in core, `cid_encoding` in render); `FontInfo` does not carry it, to keep that public struct unchanged. One behaviour moved for Identity-H: an odd byte left at the end of a string was read as a one-byte code; it is now an invalid code (CID 0, its width), as the spec says.
- Widths are read in core, the same for text extraction, the raster renderer and the SVG renderer (#222): `font::descendant_font(dict, resolve)` takes the first `/DescendantFonts` element (the array and the element each resolved when indirect, a direct dictionary accepted), `resolve_font_entries` resolves `/W` (each element, and each value inside a `c [w1 w2 …]` list) and `/DW`, and `parse_font_info` on a `CIDFontType0`/`CIDFontType2` dictionary returns `FontWidths::CID` (`parse_cid_widths`): both `/W` forms, `/DW` 1000 when absent, and a non-number inside a list 0 wide so the widths after it keep their CIDs — what MuPDF's `pdf_array_get_int` gives (`source/pdf/pdf-font.c`, read raw 2026-10-08). Until #222 only text extraction read `/W` and `/DW`; it dropped a non-number from a list, shifting the rest by one CID, and read a descendant only when it was a reference, while both renderers advanced every glyph by 1000. A descendant reference that does not resolve now leaves the Type0 font with its own (default) widths; before, the renderers failed to resolve the whole font and drew none of its text.
- `resolve_type0_descendant` still forces `FontInfo::encoding` to `Identity` for a Type0 font; that field only drives the decoding fallback for a code ToUnicode does not map.
- Vertical metrics (`/W2`, `/DW2`) are not read.
- `/CIDToGIDMap` is interpreted separately on the render side (`parse_cid_to_gid_map`, `parse_cid_gid_stream`) and in compress subsetting (`cid_font_glyph_ids`, [Font subsetting](font-subsetting.md)). Text extraction does not need it.

## Code
- `justpdf-core/src/font/mod.rs` — `descendant_font`, `parse_cid_widths`, `resolve_font_entries`
- `justpdf-core/src/font/encoding_cmap.rs` — `EncodingCMap`, `CMapCode`, `type0_encoding_cmap`, `next_code`, `invalid_code_len`, `Ranges`, `named_cmap`, `embedded_cmap`
- `justpdf-core/tests/cid_cmap.rs` — `a_code_advances_by_the_width_of_the_cid_the_cmap_maps_it_to`, `mixed_one_and_two_byte_codes_split_by_the_codespace`, `identity_h_is_unchanged`
- `justpdf-render/tests/render_cid_cmap.rs` — `a_raster_box_is_as_wide_as_the_mapped_cids_width`, `an_svg_placeholder_is_as_wide_as_the_mapped_cids_width`, `the_raster_glyph_comes_from_cid_to_gid_map_at_the_mapped_cid`
- `justpdf-core/src/content/interpret.rs` — `resolve_type0_descendant`, `show_string`
- `justpdf-core/tests/cid_widths.rs` — `both_w_forms_place_the_glyphs`, `an_indirect_width_inside_a_w_list_is_read`, `a_non_number_in_a_w_list_keeps_the_widths_after_it_in_place`, `a_direct_descendant_font_is_read`
- `justpdf-render/tests/render_cid_widths.rs` — `a_raster_box_is_as_wide_as_w`, `raster_glyphs_advance_by_both_w_forms`, `an_svg_placeholder_is_as_wide_as_w`
- `justpdf-render/src/interpreter.rs` — `parse_cid_to_gid_map`, `parse_cid_gid_stream`, `render_text_string`

## Reference behaviour
**None.** Clause to compare against: ISO 32000-2 §9.7.

## Cross-cutting invariants
- [Font resolution](../invariant/font-resolution.md) — widths are shared since #222; GID mapping is still only in render.

## Blast radius
- [Text extraction](text-extraction.md), [Glyph rendering](glyph-rendering.md) — the two implementations.
- [Font loading](font-loading.md) — fills in CID widths (`parse_font_info` on a CIDFont dictionary).
- [Font subsetting](font-subsetting.md) — updating a CID font's `/CIDToGIDMap`/`/W`.
- [CJK font embedding](cjk-font-embedding.md) — this path reads the CID fonts the writing side makes.

## Known holes / open
- Until #222 a glyph with `/W` 2000 at 10 pt extracted with width 20.0 but rendered a 10-unit placeholder (measured 2026-10-06, synthetic Type0 page, during #45's triage); `tests/render_cid_widths.rs` now pins the 20-unit box.
