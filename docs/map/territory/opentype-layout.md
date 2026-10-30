# OpenType layout

## What it is
`font::opentype` parses an sfnt's GSUB and GPOS tables into an `OpenTypeLayout`: the script, language-system and feature lists, GSUB lookups of types 1–4 (single, multiple, alternate, ligature) and GPOS pair adjustment (type 2) as kerning pairs. Nothing applies the result: there is no shaping step that uses it.

## Governing decisions
**None.** No decision record. #173's grilling (2026-10-02) settled that embedded-font text maps characters one-to-one with no shaping, and left shaping to #177; that decides the writing path, not this parser.

## Design model
Status: from reading the source; no record.

- `parse_opentype_layout(font_data)` finds `GSUB` and `GPOS` in the table directory at offset 0 (`find_table`), so it reads a single font, not a face inside a `.ttc` (inferred).
- GSUB lookup types 1–4 are parsed; any other type, including type 7 (extension), is `GsubLookup::Unsupported`. GPOS lookup type 2 is parsed; any other type, including type 9 (extension), is `GposLookup::Unsupported` (inferred, read from `parse_gsub_lookup`, `parse_gpos_lookup`). Fonts that wrap their lookups in extension subtables therefore yield no substitutions or kerning (inferred).
- GPOS pair adjustment format 2 (class-based) is skipped (code comment "skip for now, store nothing"); only format 1 pairs give `KerningPair`s, and only their `XAdvance` is kept (`read_x_advance`).
- Lookup flags are read and ignored (`_flags`).
- **No consumer.** `git grep parse_opentype_layout` outside `font/opentype.rs` finds nothing at 88b2a08 (measured 2026-10-30).
- **Input bound.** The font bytes would come from a caller or a file. Every read is bounds-checked (`read_u16` and friends return `None`); counts are bounded only by the data's length (inferred).

## Code
- `justpdf-core/src/font/opentype.rs` — `parse_opentype_layout`, `OpenTypeLayout`, `GsubTable`, `GposTable`, `ScriptRecord`, `LangSys`, `FeatureRecord`, `GsubLookup`, `LigatureSet`, `Ligature`, `GposLookup`, `KerningPair`, `find_table`, `parse_coverage`, `parse_gsub_lookup`, `parse_gpos_lookup`, `parse_gpos_pair`, `read_x_advance`

## Reference behaviour
**None.** Not compared against the OpenType specification or a shaper.

## Cross-cutting invariants
**None.**

## Blast radius
- [CJK font embedding](cjk-font-embedding.md) — the writing path that would apply ligatures and kerning if shaping is built (#177); today it maps characters one-to-one, so a change here changes no output.
- [Text wrapping](text-wrapping.md) — measures text without kerning; it would have to use these pairs for widths to match shaped text.

## Known holes / open
- No consumer in product code (measured, `git grep`).
- No shaping, ligatures or kerning on the writing side. Tracked: #177.
- Extension lookups (GSUB 7, GPOS 9) and class-based pair adjustment are not read (inferred).
