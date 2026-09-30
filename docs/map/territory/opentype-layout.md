# OpenType layout

## What it is
Parses GSUB (types 1–4) and GPOS (pair adjustment, type 2) into substitution lookups and kerning pairs.

## Governing decisions
**None.**

## Design model
- GPOS pair adjustment Format 2 (class-based) is skipped ("skip for now, store nothing").

## Code
- `justpdf-core/src/font/opentype.rs` — `parse_opentype_layout`, `GsubLookup`, `GposLookup`, `KerningPair`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Text wrapping](text-wrapping.md), [Document builder](document-builder.md) — candidates on the writing side that would apply kerning and ligatures (not connected today).

## Known holes / open
- No consumer in product code.
