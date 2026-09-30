# Tokenizer

## What it is
Reads the body of a PDF file (object syntax) byte by byte and splits it into tokens (numbers, strings, names, delimiters, keywords). It skips whitespace and comments, and interprets escapes in literal/hex strings and names. It is the common entry point of the file structure layer ([Object model](object-model.md), [Xref](xref.md), [Repair](repair.md), [Linearization](linearization.md)); content streams do not use this tokenizer.

## Governing decisions
**None.** [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) only decides that this code belongs to `justpdf-core`. There is no record deciding the tokenization rules themselves.

## Design model
Rules read out of the code (no higher-level design document).
- A word outside the known keyword set is an error. So content stream operators cannot be read with this tokenizer, and [Content stream parsing](content-stream-parsing.md) has a separate lexer that borrows only the character classification functions.
- A hex string with an odd number of digits gets a 0 appended.
- Line breaks (CR, CRLF) inside a literal string are normalized to `\n`. This is the behaviour ISO 32000 §7.3.4.2 requires. **This is exactly where the round trip breaks if the write side writes CR into a literal as is** — the read-side half of [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md). [Object serialization](object-serialization.md) writes CR as `\r` (#28).
- An unknown escape drops only the backslash.
- Integer text beyond i64 (`100000000000000000000`) is read as the nearest `Real` (`test_integer_text_beyond_i64_reads_as_real`). Before #84 it was an `invalid integer` error, so such files written by other tools could not be opened. **Maintainer decision (2026-09-25, #84)**: read it as an approximate real, as pdf.js does. Alternatives offered: truncate while reading (wrap), as MuPDF and Acrobat do; saturate at i64::MAX/MIN. [Content stream parsing](content-stream-parsing.md) follows the same rule.

## Code
- `justpdf-core/src/tokenizer/mod.rs` — `Tokenizer`, `next_token`, `seek`, `read_literal_string`, `read_hex_string`, `read_name`, `classify_keyword`
- `justpdf-core/src/tokenizer/token.rs` — `Token`, `Keyword`
- `justpdf-core/src/tokenizer/reader.rs` — `PdfReader`, `is_pdf_whitespace`, `is_pdf_delimiter`, `is_pdf_regular`

## Reference behaviour
**None.** No record of a comparison with a reference. Clauses to compare against: ISO 32000-2 §7.2 (lexical conventions), §7.3.4 (strings), §7.3.5 (names) — clause pointers from memory, not checked against the spec text.

## Cross-cutting invariants
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md) — every piece of syntax the write side produces must read back through this tokenizer as the same value.

## Blast radius
- [Object model](object-model.md) — if token shapes change, the reference (`N M R`) rewind logic in `parse_object` is affected.
- [Object serialization](object-serialization.md) — changing the string or name interpretation rules means aligning the write-side escape rules too.
- [Xref](xref.md), [Repair](repair.md), [Linearization](linearization.md) — all read the file structure with this tokenizer.
- [Content stream parsing](content-stream-parsing.md) — shares the character classification functions of `reader`.

## Known holes / open
**None.** No known holes — every hole this note was tracking has been fixed.
