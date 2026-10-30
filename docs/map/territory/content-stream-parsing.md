# Content stream parsing

## What it is
Parses the bytes of a content stream — a page, a Form XObject, an appearance stream, a pattern tile — into a list of operations (`ContentOp { operator, operands }`), and writes such a list back to bytes. An inline image becomes one `BI` operation carrying an `Operand::InlineImage`. Every operator walker in the workspace takes this list as its input: the [Content interpreter](content-interpreter.md) (text extraction), the raster and SVG renderers, the bbox device, redaction, compression and font subsetting; the CID-font code also runs embedded CMap streams through it.

## Governing decisions
**None.** The writing side (`ContentOp::write_to`, `Operand::write_to`, `write_content`) stays `pub(crate)` by a maintainer decision (2026-09-25, #29 check-it): its callers are all inside `justpdf-core`, and the inverse property below holds only for operations this parser produced. Making it public later is non-breaking. No ADR covers this area.

## Design model
From reading the source and the unit tests in `content/mod.rs`; nothing in a record.
- **A lexer of its own**, separate from the file-structure [Tokenizer](tokenizer.md); only the character classes (`is_pdf_whitespace`, `is_pdf_delimiter`, `is_pdf_regular`) are shared (imports in `content/mod.rs`).
- **Tokens**: `#XX` in names is decoded (`read_name`). A byte that starts no token is skipped. A word that is not `true`, `false` or `null` ends the operation as its operator; inside an array or a dictionary such a word becomes a `Name`. Operands left at the end of the stream with no operator are dropped. Operands before a `BI` are dropped too: `next_op` returns the `BI` operation with only the inline image (inferred).
- **Literal strings**: an unescaped CR or CRLF reads as LF, as ISO 32000-1 §7.3.4.2 requires and as the tokenizer does (`test_literal_line_ends_read_as_line_feed`, both parsers, #87).
- **Numbers**: integer text beyond `i64` reads as the nearest `Real`, the tokenizer's rule (`test_integer_text_beyond_i64_reads_as_real`, both parsers, #84; before #84 it was `Integer(0)`). Exponent notation is not a number token (`read_number` takes digits, sign and one dot).
- **Inline images**: dictionary values are read like those of any dictionary — `<<` is a dictionary, `null` is `Null` (`test_inline_image_dict_values_read_like_other_dicts`, both parsers, #29; before #29 `<<` read as a hex string). After `ID` one whitespace byte is skipped. The data ends at the first `EI` that has whitespace before it and whitespace, a delimiter or the end after it, and the whitespace byte before `EI` is not data. An image with no such `EI` gets empty data and swallows the rest of the stream (inferred from `read_inline_image`).
- **The writing side is the inverse for what this parser produced**: writing a list the parser produced with `write_content` and parsing it again gives the same list (`test_written_ops_read_back_unchanged`, #29). A hand-built operation need not survive: inline data containing whitespace, `EI` and whitespace is cut there on reading. An inline image operand under an operator other than `BI` is written with that operator after it (`test_inline_image_under_another_operator_keeps_the_operator`). Names, strings and reals go through the same functions as `PdfObject` display (`write_name`, `write_string`, `write_real`); non-finite reals are written as finite ones (`test_non_finite_operands_are_written_as_finite_reals`). An inline image is written `BI <entries> ID <data> EI` with one space on each side of the data, the two bytes the parser strips. `Operand::Dict` is written in its own order without going through `PdfDict`, so the key order of `BDC` property lists is kept — [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md).
- **It never fails**: `parse_content_stream` returns `Result`, but no path in `content/mod.rs` produces an `Err` (inferred, `git grep -n Err justpdf-core/src/content/mod.rs` outside tests finds none). Malformed input gives whatever operations could be read.
- `ContentOp`'s `Display` is `write_to` decoded lossily as UTF-8; code that writes syntax uses `write_to`, not `Display`.
- **Two parsers**: `ContentParser` (always) and `ArenaContentParser` behind feature `arena` (`parse_content_stream_arena`), the same grammar copied; nothing outside `justpdf-core`'s own tests calls the arena entry (`git grep parse_content_stream_arena`).
- **Bounds on input**: the parser takes the whole decoded stream; how large that can get is decided by stream decoding (#161, [Stream filters](stream-filters.md)). The number of operations and operands is not bounded. Array and dictionary nesting is not bounded either: `read_array` and `read_inline_dict` recurse once per level, so deep nesting overflows the stack and aborts the process (measured 2026-10-30 with a probe calling `parse_content_stream` on `[`×n then ` 0 Tj`: release build fine at 10,000 and aborted at 100,000; debug build aborted at 10,000).

## Code
- `justpdf-core/src/content/mod.rs` — `parse_content_stream`, `parse_content_stream_arena`, `ContentParser`, `ContentParser::next_op`, `ContentParser::read_number`, `ContentParser::read_literal_string`, `ContentParser::read_name`, `ContentParser::read_array`, `ContentParser::read_inline_dict`, `ContentParser::read_inline_image`, `ArenaContentParser`
- `justpdf-core/src/content/operator.rs` — `Operand`, `ContentOp`, `ContentOp::write_to`, `Operand::write_to`, `write_content`

## Reference behaviour
**None.** No comparison with MuPDF's lexer (`pdf-lex.c`) is recorded. Clauses to compare against: ISO 32000-2 §7.8.2 (content streams), §8.9.7 (inline images), Annex A (operator summary).

## Cross-cutting invariants
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md) — `write_content`, `Operand::write_to`.
- [Page content assembly](../invariant/page-content-assembly.md) — each consumer builds this parser's input from `/Contents` in its own way.

## Blast radius
- [Content interpreter](content-interpreter.md), [Render interpreter](render-interpreter.md), [SVG renderer](svg-renderer.md), [BBox device](bbox-device.md) — they read operands by shape (`as_f64`, `as_name`, `as_str`, `as_array`); a token read as a different `Operand` variant is silently ignored by all of them.
- [Redaction](redaction.md) — parses, filters, and writes back with `write_content`; it relies on the inverse property, so an operation that does not read back the same changes the page beyond the erased glyphs.
- [Compress grayscale](compress-grayscale.md) — rewrites colour operators and writes each operation back with `ContentOp::write_to`; same reliance on the inverse.
- [Form appearance](form-appearance.md) — parses a `/DA` string and writes it back with `write_content` (`FieldFont::select_at`, then decoded lossily as UTF-8), so a non-UTF-8 operand there is altered.
- [Compress images](compress-images.md), [Compress unused resources](compress-unused-resources.md), [Font subsetting](font-subsetting.md) — read operations to collect the CTM, resource names and shown codes; each treats a parse error as "skip this stream", an arm that cannot run today (above), so making the parser return errors changes what each of them keeps.
- [CID fonts](cid-fonts.md) — an embedded CMap stream is parsed with this lexer (`font/encoding_cmap.rs`); CMap syntax that the content grammar reads differently changes code-to-CID mapping.
- When adding an `Operand` variant, update `Operand::write_to` (the `match` makes a missing variant a compile error) and `optional_content::operand_to_pdf_object` in `justpdf-render`.

## Known holes / open
- Array and dictionary nesting has no depth limit; a crafted content stream of about 100 KB of `[` aborts the process in any consumer (measured, above). Not tracked by an issue.
- An inline image with no terminating `EI` silently takes the rest of the stream as consumed and yields empty data (inferred).
- Operands written before `BI` are discarded (inferred).
