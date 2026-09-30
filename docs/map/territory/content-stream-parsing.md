# Content stream parsing

## What it is
Parses the content of pages, Form XObjects and appearance streams into an operator list (`ContentOp { operator, operands }`). An inline image becomes a single `BI` operation carrying an `Operand::InlineImage`. The renderer, text extraction, redaction and compression all take this result as input. `ContentOp::write_to`/`Operand::write_to`/`write_content` (`pub(crate)`) are the side that writes operators back to bytes. **Maintainer decision (2026-09-25, #29 check-it)**: do not make them public — facts offered: the only callers are inside the crate, and the inverse is guaranteed only for operations the parser produced (below). Making them public later is non-breaking.

## Governing decisions
**None.**

## Design model
- A lexer separate from the file-structure [Tokenizer](tokenizer.md) (only the character classification functions are shared).
- Decodes `#XX` in names. EI must be preceded by whitespace. Unknown bytes are skipped.
- Values in an inline image dictionary are read the same way as in `read_inline_dict` and `read_array` — `<<` is a dictionary, `null` is `Null`. Before #29, `<<` was read as a hex string, so `/DP << /Predictor 15 … >>` became a garbage string (`test_inline_image_dict_values_read_like_other_dicts`, both the plain and arena parsers).
- **The writing side is the inverse of the operations this parser produces**: writing a `ContentOp` list this parser produced with `write_content` and reading it back gives the same list (`test_written_ops_read_back_unchanged`, #29). Hand-built operations may not — if inline image data contains whitespace + `EI` + whitespace/delimiter, the parser cuts the image there (data this parser produces never has that pattern). Putting an inline image operand on an operator other than `BI` writes that operator after the image. Names, strings and reals are written with the same functions as `PdfObject` Display (`write_name`, `write_string`, `write_real`). An inline image is written as `BI <dict> ID <data> EI` with the data as raw bytes — the parser strips one whitespace after `ID` and one before `EI`, so those two are written. A dictionary (`Operand::Dict`) is written without converting it to `PdfDict` (`BTreeMap`) — converting would change the key order of `BDC` property dictionaries — [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md).
- `ContentOp`'s Display shows the result of `write_to`. It is a `String`, so non-UTF-8 inline image data shows up lossily; code that writes syntax uses `write_to`, not Display.
- Unescaped CR and CRLF inside a literal string are read as LF — the same as the tokenizer, following ISO 32000-1 §7.3.4.2 ("An end-of-line marker appearing within a literal string without a preceding REVERSE SOLIDUS shall be treated as a byte value of (0Ah)") (`test_literal_line_ends_read_as_line_feed`, both the plain and arena parsers, #87). Before #87 CR was kept as is, so tests that read back through this parser could not see a missing CR escape on the writing side.
- Integer text beyond i64 is read as the nearest `Real` — the same rule as the [Tokenizer](tokenizer.md) (both parsers, `test_integer_text_beyond_i64_reads_as_real`, #84). Before #84 it was `Integer(0)` (`parse().unwrap_or(0)`).
- The arena parser (`parse_content_stream_arena`, feature `arena`) is not called from outside core.

## Code
- `justpdf-core/src/content/mod.rs` — `parse_content_stream`, `parse_content_stream_arena`, `ContentParser`, `next_op`, `read_name`, `read_inline_image`
- `justpdf-core/src/content/operator.rs` — `Operand`, `ContentOp`, `ContentOp::write_to`, `Operand::write_to`, `write_content`

## Reference behaviour
**None.** Clauses to compare against: ISO 32000-2 §7.8.2, Annex A (operator summary).

## Cross-cutting invariants
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md) — `write_content`.
- [Page content assembly](../invariant/page-content-assembly.md) — each consumer has its own way of building the parser's input.

## Blast radius
- [Render interpreter](render-interpreter.md), [SVG renderer](svg-renderer.md), [BBox device](bbox-device.md) — consumers of `ContentOp`.
- [Text extraction](text-extraction.md) — the same operator stream.
- [Redaction](redaction.md) — parses, then writes back with `write_content`.
- [Compress grayscale](compress-grayscale.md) — parses, then writes back with `ContentOp::write_to`.
- [Compress images](compress-images.md), [Compress unused resources](compress-unused-resources.md) — collect the CTM and resource usage.
- When adding an `Operand` variant, update `Operand::write_to` (the `match` reports a missing variant as a compile error).

## Known holes / open
**None.** No known holes — every hole this note was tracking has been fixed.
