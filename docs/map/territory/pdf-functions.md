# PDF functions

## What it is
Parser and evaluator for PDF function objects, used by shadings, tint transforms and soft-mask transfer functions. Supports sampled (Type 0), exponential (Type 2), stitching (Type 3) and PostScript calculator (Type 4).

## Governing decisions
**None.** The split below was settled at #47's triage (2026-10-06): resolving references and decoding streams belongs in core, not in each caller.

## Design model
- **An array of functions is `PdfFunction::Array`** (#233): `resolve` and `parse` read an array of functions, and `evaluate` concatenates their outputs for the same inputs (a shading's `/Function` as one 1-out function per colour component, ISO 32000-2 §8.7.4.5).
- **`PdfFunction::resolve(obj, doc)` reads a document's function** (#47): it resolves references, decodes stream data through `doc.decode_stream` (filters, and the decryption `resolve` already applies), and reads stitching `/Functions` entries the same way, to depth 8 (`MAX_DEPTH`), so a function that names itself stops instead of overflowing the stack (measured by mutation: removing the cap overflows `a_stitching_function_naming_itself_stops`). `parse(obj)` reads a function as given, without following references or decoding: callers with a document use `resolve` (tint transforms in `ColorSpace::resolve`, `/TR` in the renderer's `transfer_table`, function-based shading).
- **Type 0 (sampled)** follows ISO 32000-1 §7.10.2 (read raw): each input clipped to its `/Domain` pair, mapped through `/Encode` (default `[0, Size_i − 1]`) and clipped to the table, the surrounding samples interpolated multilinearly (2^m corners, the first input varying fastest), each output mapped from `[0, 2^BitsPerSample − 1]` through `/Decode` (default `/Range`) and clipped to `/Range`. Samples form one bit stream with no padding; `/BitsPerSample` 1, 2, 4, 8, 12, 16, 24 or 32. Bits past the end read as 0, so a truncated table does not panic. `/Order 3` is interpolated linearly. More than 16 inputs return zeros.
- **Evaluation never panics** on malformed input: each PostScript input is clamped to its own `/Domain` pair or `[0, 1]`, and every clamp orders its bounds and ignores a NaN bound (`clamp_between`; a reversed `[1 0]` made `f64::clamp` panic). Until #47 an input past the `/Domain` pairs panicked (`range end index 2 out of range for slice of length 1`, measured 2026-10-06), Type 0 did not parse, a Flate-filtered Type 4 did not parse, and a referenced stitching sub-function was dropped.
- Real files (measured 2026-10-25, 18 local PDFs, 100 pages): pixel-identical to master; none uses these functions.
- Pinned by `justpdf-core/tests/pdf_function.rs` and `justpdf-render/tests/render_pdf_functions.rs`; mutations dropping decoding, sub-function resolution, interpolation, bit reading, `/Encode`, `/Decode`, bound ordering, the per-input domain or the depth cap each fail a named test.

## Code
- `justpdf-core/tests/pdf_function.rs` — Type 0, filtered Type 4, referenced stitching, no-panic cases
- `justpdf-core/src/function.rs` — `PdfFunction`, `PsOp`, `parse`, `resolve`, `evaluate`, `evaluate_sampled`, `read_bits`, `clamp_between`, `parse_ps_code`, `execute_ps_ops`, `clamp_input`, `clamp_output`

## Reference behaviour
ISO 32000-1 §7.10.2 for Type 0 (read raw for #47). The other types follow the code's own reading of §7.10.

## Cross-cutting invariants
**None.**

## Blast radius
- [Render shading](render-shading.md) — every shading type evaluates its `/Function` through `PdfFunction` (#233).

## Known holes / open
- **None.**
