# Type3 fonts

## What it is
Parses a Type3 font dictionary (`/CharProcs`, `/FontMatrix`, `/Encoding /Differences`, widths) and finds character code → glyph name → CharProc stream.

## Governing decisions
**None.**

## Design model
- The only `/Differences` parser in the repository is here (Type3 only).

## Code
- `justpdf-core/src/font/type3.rs` — `parse_type3_font`, `Type3Font`, `glyph_name`, `char_proc`, `resolve_char_proc`, `DEFAULT_FONT_MATRIX`

## Reference behaviour
**None.** Clause to compare against: ISO 32000-2 §9.6.4.

## Cross-cutting invariants
**None.**

## Blast radius
- [Render interpreter](render-interpreter.md) — `d0`/`d1` are no-ops and CharProcs are not run. This is where Type3 rendering would go.
- [Text extraction](text-extraction.md) — divides widths by 1000 and ignores `/FontMatrix` (inferred).
- [Font encodings](font-encodings.md) — the candidate for moving the `/Differences` parser to general simple fonts.

## Known holes / open
- There is no consumer in product code.
- Tracked: #226 (rendering CharProcs), #221 (promoting the `/Differences` parser)
