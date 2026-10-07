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
- [Glyph rendering](glyph-rendering.md) — renders bare CFF through `ttf_parser::cff::Table`, not through this parser.
- [Font subsetting](font-subsetting.md) — if CFF subsetting is added, this is its material.

## Known holes / open
- There is no consumer in product code. The renderer draws bare CFF through `ttf_parser::cff::Table` (#224), so this parser still has no renderer consumer; #176 is its candidate consumer.
- Test fixtures for bare CFF live in `justpdf-render/tests/fixtures/`: URW `NimbusSans-Regular.cff` (from MuPDF `resources/fonts/urw`, SIL OFL 1.1, `OFL-URW.txt`), and `cid-test.cff` / `cid-test-half.cff`, CID-keyed fonts built from it by `scripts/make-cid-cff-fixture.py` (fontTools; H = CID 300, E = CID 301, Top DICT FontMatrix 0.001 and 0.0005, no FDArray FontMatrix).
- Tracked: #176 (CFF on the writing side)
