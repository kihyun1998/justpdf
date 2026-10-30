# PDF functions

## What it is
Parser and evaluator for PDF function objects: sampled (Type 0), exponential (Type 2), stitching (Type 3), PostScript calculator (Type 4), and an array of functions. Shadings, Separation/DeviceN tint transforms and the soft-mask transfer function `/TR` evaluate through it.

## Governing decisions
**None.** Decided at #47's triage (2026-10-06): resolving references and decoding streams belongs in core, not in each caller.

## Design model
Rules read from the code (*inferred* unless marked).
- **An array of functions is `PdfFunction::Array`** (#233): `resolve` and `parse` read an array of functions, and `evaluate` concatenates their outputs for the same inputs (a shading's `/Function` given as one 1-out function per colour component, ISO 32000-2 §8.7.4.5). An empty array is no function.
- **`PdfFunction::resolve(obj, doc)` reads a document's function** (#47): it resolves references, decodes stream data through `PdfDocument::decode_stream` (filters; decryption is already applied by `resolve`), and reads array members and stitching `/Functions` entries the same way, to depth 8 (`MAX_DEPTH`), so a function that names itself stops instead of overflowing the stack (*measured* for #47 by mutation: removing the cap overflows `a_stitching_function_naming_itself_stops`). `parse(obj)` reads a function as given, without following references or decoding. Product callers use `resolve`: tint transforms in `ColorSpace::resolve`, `/TR` in the renderer's `transfer_table`, and shading `/Function` in the raster renderer. `parse` is reached from `ColorSpace::from_pdf_object`.
- **Type 0 (sampled)** follows ISO 32000-1 §7.10.2 (read raw for #47): each input clipped to its `/Domain` pair, mapped through `/Encode` (default `[0, Size_i − 1]`) and clipped to the table, the surrounding samples interpolated multilinearly (2^m corners, the first input varying fastest), each output mapped from `[0, 2^BitsPerSample − 1]` through `/Decode` (default `/Range`) and clipped to `/Range`. Samples form one bit stream with no padding; `/BitsPerSample` 1, 2, 4, 8, 12, 16, 24 or 32, else no function. A `/Size` entry below 1 reads as 1. Bits past the end read as 0, so a truncated table does not panic. `/Order 3` is interpolated linearly. More than 16 inputs return zeros.
- **Evaluation never panics** on malformed input: each PostScript input is clamped to its own `/Domain` pair or `[0, 1]`, and every clamp orders its bounds and ignores a NaN bound (`clamp_between`; a reversed `[1 0]` made `f64::clamp` panic). Division and modulo by zero push 0. Until #47 an input past the `/Domain` pairs panicked (`range end index 2 out of range for slice of length 1`, *measured* 2026-10-06), Type 0 did not parse, a Flate-filtered Type 4 did not parse, and a referenced stitching sub-function was dropped.
- Real files (*measured* 2026-10-25, 18 local PDFs, 100 pages): pixel-identical to master after #47; none uses these functions.
- Pinned by `justpdf-core/tests/pdf_function.rs` and `justpdf-render/tests/render_pdf_functions.rs`; mutations dropping decoding, sub-function resolution, interpolation, bit reading, `/Encode`, `/Decode`, bound ordering, the per-input domain or the depth cap each failed a named test (#47).
- **Input bound:** nesting of `resolve` is capped at 8. A Type 0 table is read by index with saturating arithmetic and never allocated by `/Size`. The PostScript parser recurses per `{ }` block (`parse_ps_block`) with no depth limit, and the operand stack has no size limit: `copy` pushes as many values as its operand names, so each `copy` can double the stack (inferred, not run).

## Code
- `justpdf-core/src/function.rs` — `PdfFunction`, `PsOp`, `MAX_DEPTH`, `PdfFunction::parse`, `PdfFunction::resolve`, `PdfFunction::resolve_at`, `PdfFunction::from_object`, `PdfFunction::parse_sampled`, `PdfFunction::parse_exponential`, `PdfFunction::parse_stitching`, `PdfFunction::parse_postscript`, `PdfFunction::evaluate`, `evaluate_sampled`, `read_bits`, `clamp_between`, `clamp_input`, `clamp_output`, `parse_ps_code`, `parse_ps_block`, `tokenize_ps`, `extract_block`, `execute_ps_ops`
- `justpdf-core/tests/pdf_function.rs` — Type 0, filtered Type 4, referenced stitching, no-panic cases, `a_stitching_function_naming_itself_stops`

## Reference behaviour
- ISO 32000-1 §7.10.2 for Type 0 (read raw for #47, 2026-10-06). The other types follow the code's own reading of §7.10; not compared.

## Cross-cutting invariants
**None.**

## Blast radius
- [Render shading](render-shading.md) — every shading type evaluates its `/Function` through `PdfFunction` (#233); a change to output count or clamping changes every shaded colour.
- [Color spaces](color-spaces.md) — Separation and DeviceN convert through their tint transform; a function that stops parsing turns the tint into gray.
- [Render transparency](render-transparency.md) — a soft mask's `/TR` is evaluated into a 256-entry table (`transfer_table`); a function that stops parsing is treated as identity.
- [Stream filters](stream-filters.md) — Type 0 and Type 4 data is decoded there; a decode failure makes the function absent.

## Known holes / open
- The PostScript calculator parser has no depth limit on nested blocks (inferred). #122's brief lists it among the recursive parsers to check under the 256 limit. Tracked: #122
- The PostScript operand stack has no size limit (inferred above). No issue tracks it.
