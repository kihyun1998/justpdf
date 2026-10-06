# PDF functions

## What it is
Parser and evaluator for PDF function objects, used by shadings, transfer functions and so on. Supports exponential (Type 2), stitching (Type 3) and PostScript calculator (Type 4).

## Governing decisions
**None.**

## Design model
- **There is no Type 0 (sampled) function.** `parse` accepts only 2, 3 and 4.
- Input clamping uses only the first Domain pair. The PostScript evaluator can index past the slice when there are more inputs than Domain pairs (inferred: panic).
- A stitching sub-function that is a reference is filtered out (only `Dict`/`Stream` accepted — inferred).
- PostScript code does `from_utf8` on the raw `Stream.data` bytes. The renderer passes the stream without decoding it, so a Flate-compressed Type 4 fails to parse (inferred).

## Code
- `justpdf-core/src/function.rs` — `PdfFunction`, `PsOp`, `parse`, `evaluate`, `parse_ps_code`, `execute_ps_ops`, `clamp_input`, `clamp_output`

## Reference behaviour
**None.** The code cites "PDF 2.0 spec, section 7.10" (not a comparison record).

## Cross-cutting invariants
**None.**

## Blast radius
- [Render shading](render-shading.md) — only function-based shading uses `PdfFunction`; axial and radial shading read C0/C1/Bounds again by hand. Wider function support does not reach axial and radial (tracked: #233).

## Known holes / open
- There is no stitching function test.
- Tracked: #47 (function Type 0 and Type 4)
