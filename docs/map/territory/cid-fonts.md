# CID fonts (reading Type0)

## What it is
The path that reads Type0 composite fonts and their descendant CIDFont: splitting 2-byte codes, `/W` and `/DW` widths, `/CIDToGIDMap`. Text extraction and the renderer each implement it.

## Governing decisions
**None.**

## Design model
- Both consumers **hard-code the code width to 2 bytes**: text uses `Identity || Subtype == Type0`, render uses `Subtype == Type0`. Variable-width CMaps (e.g. mixed 1/2 bytes) are not supported (inferred).
- `/W` parsing exists only on the text side (`parse_cid_widths`, `resolve_type0_descendant`). `resolve_type0_descendant` forces the encoding to `Identity`.
- `/CIDToGIDMap` is interpreted separately on the render side (`parse_cid_to_gid_map`, `parse_cid_gid_stream`) and in compress subsetting (`cid_font_glyph_ids`, [Font subsetting](font-subsetting.md)). Text extraction does not need it.

## Code
- `justpdf-core/src/text/mod.rs` — `parse_cid_widths`, `resolve_type0_descendant`, `show_string`
- `justpdf-render/src/interpreter.rs` — `parse_cid_to_gid_map`, `parse_cid_gid_stream`, `render_text_string`

## Reference behaviour
**None.** Clause to compare against: ISO 32000-2 §9.7.

## Cross-cutting invariants
- [Font resolution](../invariant/font-resolution.md) — this note is the prime example of the mismatch (widths only in text, GID mapping only in render).

## Blast radius
- [Text extraction](text-extraction.md), [Glyph rendering](glyph-rendering.md) — the two implementations.
- [Font loading](font-loading.md) — the side that does not fill in CID widths.
- [Font subsetting](font-subsetting.md) — updating a CID font's `/CIDToGIDMap`/`/W`.
- [CJK font embedding](cjk-font-embedding.md) — this path reads the CID fonts the writing side makes.

## Known holes / open
- A font's width information does not reach the renderer (the renderer advances by a fixed width of 1000 — inferred).
- Tracked: #45 (/Differences, CID widths, CFF)
