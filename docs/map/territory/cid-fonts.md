# CID fonts (reading Type0)

## What it is
The path that reads Type0 composite fonts and their descendant CIDFont: splitting 2-byte codes, `/W` and `/DW` widths, `/CIDToGIDMap`. Widths are read once in core; code splitting and `/CIDToGIDMap` are still implemented by each consumer.

## Governing decisions
**None.**

## Design model
- Both consumers **hard-code the code width to 2 bytes**: text uses `Identity || Subtype == Type0`, render uses `Subtype == Type0`. Variable-width CMaps (e.g. mixed 1/2 bytes) are not supported (inferred).
- Widths are read in core, the same for text extraction, the raster renderer and the SVG renderer (#222): `font::descendant_font(dict, resolve)` takes the first `/DescendantFonts` element (the array and the element each resolved when indirect, a direct dictionary accepted), `resolve_font_entries` resolves `/W` (each element, and each value inside a `c [w1 w2 …]` list) and `/DW`, and `parse_font_info` on a `CIDFontType0`/`CIDFontType2` dictionary returns `FontWidths::CID` (`parse_cid_widths`): both `/W` forms, `/DW` 1000 when absent, and a non-number inside a list 0 wide so the widths after it keep their CIDs — what MuPDF's `pdf_array_get_int` gives (`source/pdf/pdf-font.c`, read raw 2026-10-08). Until #222 only text extraction read `/W` and `/DW`; it dropped a non-number from a list, shifting the rest by one CID, and read a descendant only when it was a reference, while both renderers advanced every glyph by 1000. A descendant reference that does not resolve now leaves the Type0 font with its own (default) widths; before, the renderers failed to resolve the whole font and drew none of its text.
- Widths are looked up by **code**, not CID: right for `Identity-H`/`-V`, where they are equal; under any other CMap the lookup uses the wrong index (inferred). `resolve_type0_descendant` forces the encoding to `Identity`.
- Vertical metrics (`/W2`, `/DW2`) are not read.
- `/CIDToGIDMap` is interpreted separately on the render side (`parse_cid_to_gid_map`, `parse_cid_gid_stream`) and in compress subsetting (`cid_font_glyph_ids`, [Font subsetting](font-subsetting.md)). Text extraction does not need it.

## Code
- `justpdf-core/src/font/mod.rs` — `descendant_font`, `parse_cid_widths`, `resolve_font_entries`
- `justpdf-core/src/text/mod.rs` — `resolve_type0_descendant`, `show_string`
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
