# CFF parser

## What it is
`font::cff` parses the header, INDEXes, Top DICT, Private DICT widths and charset of a Compact Font Format program (`/FontFile3 /Subtype /Type1C` or CIDFontType0C) into a `CffFont`. It does not interpret Type 2 CharStrings and has no consumer in product code: bare CFF is drawn and checked through `ttf_parser::cff::Table`, which core re-exports, not through this module. This note also holds the bare-CFF test fixtures and the script that builds the CID-keyed ones.

## Governing decisions
**None.** No decision record. The maintainer's decisions on #341 (CFF glyph checks through `ttf_parser::cff::Table`, each crate keeping its own fixtures) are written below with their source.

## Design model
Status: from reading the source; no record.

- **What it reads** (`parse_cff`): the header (skipping `hdrSize`), the Name INDEX (first name), the Top DICT INDEX (first Top DICT only), the String INDEX, the Global Subr INDEX (parsed and discarded), the CharStrings INDEX (only its count; a missing or out-of-range offset returns `None`), the Private DICT's `defaultWidthX` and `nominalWidthX` (0 when it lies outside the data), and the charset (predefined ISOAdobe/Expert/ExpertSubset, or a custom format 0/1/2 SID list; an offset past the data falls back to ISOAdobe). The Top DICT gives the names, `FontBBox`, the offsets, and for a CID-keyed font the ROS, `FDArray` and `FDSelect` offsets (inferred, read from `build_top_dict`).
- It **does not interpret Type 2 CharString programs** (module doc), so it gives no outlines and no per-glyph widths. The encoding offset is read but the encoding is not parsed (inferred).
- It **does not encode CFF**. The old note's "and encodes them again" was wrong: the only encoder, `encode_dict_int`, is inside `mod tests`, where it builds test fonts (inferred, read 2026-10-30).
- **No consumer.** `git grep parse_cff` outside `font/cff.rs` finds nothing at 88b2a08 (measured 2026-10-30). The renderer (`justpdf-render/src/glyph.rs`, `GlyphSource::Cff`) and `FontEncoder` use `ttf_parser::cff::Table` with core's `cff_simple_glyph`, `cff_cid_to_gid` and `cff_has_glyph` ([Font encodings](font-encodings.md)); the renderer has drawn bare CFF that way since #224.
- **Fixtures.** `justpdf-render/tests/fixtures/` holds URW `NimbusSans-Regular.cff` (from MuPDF `resources/fonts/urw`, SIL OFL 1.1, `OFL-URW.txt`) and `cid-test.cff` / `cid-test-half.cff`. `NimbusSans-Regular.cff` and `cid-test.cff` are copied with `OFL-URW.txt` into `justpdf-core/tests/fixtures/` for the encoder's tests (`form_da_font.rs`); each crate keeps its own fixtures (maintainer decision, #341).
- **`scripts/make-cid-cff-fixture.py`** builds `cid-test.cff` and `cid-test-half.cff` from `NimbusSans-Regular.cff` with fontTools: GID 0 `.notdef` (CID 0), GID 1 `H` (CID 300), GID 2 `E` (CID 301), subroutines flattened into the charstrings, Top DICT `FontMatrix` 0.001 and 0.0005, no `FDArray` `FontMatrix` (read from the script's docstring and body 2026-10-30; not rerun).
- **Input bound.** The program comes from the file. Every read is bounds-checked against the slice and returns `None` or a fallback; nothing bounds INDEX counts beyond the data's length (inferred).

## Code
- `justpdf-core/src/font/cff.rs` — `parse_cff`, `CffFont`, `CffTopDict`, `CffCharset`, `parse_dict`, `parse_real_operand`, `build_top_dict`, `parse_private_dict`, `parse_charset`, `parse_custom_charset`, `sid_to_string`
- `scripts/make-cid-cff-fixture.py` — `flatten`
- `justpdf-core/tests/form_da_font.rs` — `NIMBUS_SANS`, `CID_TEST` (the core copies of the fixtures)
- `justpdf-render/tests/render_cff.rs` — `NIMBUS_SANS`, `CID_TEST`, `CID_TEST_HALF`

## Reference behaviour
**None.** Not compared against a reference implementation. The module cites Adobe Technical Note #5176 (the CFF specification) as its source.

## Cross-cutting invariants
**None.**

## Blast radius
- [Glyph rendering](glyph-rendering.md) — draws bare CFF through `ttf_parser::cff::Table`; a move of the renderer onto this parser would need outlines this parser does not produce.
- [Font encodings](font-encodings.md) — `FontEncoder` checks CFF glyphs through `ttf_parser::cff::Table` and uses the core copies of the fixtures; a fixture change must reach both crates.
- [CID fonts](cid-fonts.md) — `cid-test.cff` pins CID → GID through a CFF charset for the raster renderer and `FontEncoder`.
- [Font subsetting](font-subsetting.md) — has no CFF subsetting; this parser would be its starting material (#176).

## Known holes / open
- No consumer in product code; the module is reachable only as public API (`justpdf_core::font::cff`). #176 (CFF on the writing side) is its candidate consumer. Tracked: #176.
- Open question for the maintainer: keep `font::cff` as public API with no consumer, or remove it now that `ttf_parser::cff::Table` serves both the renderer and `FontEncoder`?
