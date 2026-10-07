# CFF parser

## What it is
Parses the header, INDEXes, Top DICT and charset of a Compact Font Format (FontFile3) font and encodes them again. It does not interpret Type 2 CharStrings.

## Governing decisions
**None.**

## Design model
- "does not interpret Type 2 CharString programs", and the Global Subr INDEX is skipped.

## Code
- `justpdf-core/src/font/cff.rs` — `parse_cff`, `CffFont`, `CffTopDict`, `CffCharset`

## Reference behaviour
**None.** The code cites Adobe TN #5176 (not a comparison record).

## Cross-cutting invariants
**None.**

## Blast radius
- [Glyph rendering](glyph-rendering.md) — rendering a CFF font would need this, but it currently hands the raw bytes to `ttf_parser`.
- [Font subsetting](font-subsetting.md) — if CFF subsetting is added, this is its material.

## Known holes / open
- There is no consumer in product code. The renderer hands a pure CFF (FontFile3) to `ttf_parser::Face::parse` and draws a placeholder box when that fails (measured 2026-10-07: URW `NimbusSans-Regular.cff` as `/Type1C`, `H` at 100 pt fills its counter).
- `ttf-parser` 0.25.1 exports `ttf_parser::cff::Table`, which parses a bare CFF and gives `outline`, `glyph_index`, `glyph_index_by_name`, `glyph_cid` and `matrix` (measured 2026-10-07 on the URW Sans and Dingbats files). #224 renders through it, so this parser stays without a renderer consumer; #176 is still its candidate consumer.
- Tracked: #224 (rendering FontFile3), #176 (CFF on the writing side)
