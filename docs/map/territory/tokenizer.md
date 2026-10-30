# Tokenizer

## What it is
Reads PDF object syntax byte by byte and splits it into tokens: integers, reals, literal and hex strings, names, the delimiters `[ ] << >>`, and the eleven keywords `true false null obj endobj stream endstream xref trailer startxref R` (`Keyword::from_bytes`). It skips whitespace and `%` comments and interprets the escapes in literal strings, hex strings and names. It is the shared entry of the file-structure layer: the [Object model](object-model.md) parser, [Xref](xref.md) (the trailer and xref streams), [Document access](document-access.md) and [Object streams](object-streams.md) (object loads and the ObjStm index), [Repair](repair.md) and [Linearization](linearization.md) all read through `Tokenizer` (inferred: `rg -l 'Tokenizer\b' justpdf-core/src`). Content streams do not use it.

## Governing decisions
**None.** [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) only places this code in `justpdf-core`; no record decides the tokenization rules.

## Design model
Rules read from the code (*inferred* unless marked); no design document.
- A word outside the keyword set is an `InvalidToken` error (`classify_keyword`). So content-stream operators cannot be read with this tokenizer, and [Content stream parsing](content-stream-parsing.md) has its own lexer that borrows only the byte classes `is_pdf_whitespace`, `is_pdf_delimiter`, `is_pdf_regular` (`justpdf-core/src/content/mod.rs` imports them).
- `{` and `}` are delimiters but no token: each is an `unexpected byte` error.
- A hex string with an odd number of digits gets a trailing `0` (`read_hex_string`). A non-hex byte inside one is an error.
- In a literal string, an unescaped CR or CRLF reads as `\n`, as ISO 32000-2 §7.3.4.2 requires (clause not re-read). This is the read-side half of [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md): the write side must escape a CR as `\r`, which [Object serialization](object-serialization.md) does (`object/types.rs` writes `\\r`, #28).
- An unknown escape in a literal string drops only the backslash. Parentheses nest by a `u32` depth counter.
- A name's `#XX` escape must be two hex digits, else `invalid hex escape in name`.
- A number token is a run of digits, signs and dots. With a dot it must parse as `f64`, else an error. Without one, integer text beyond `i64` reads as the nearest `Real` (`test_integer_text_beyond_i64_reads_as_real`). Before #84 it was an `invalid integer` error, so such files from other tools did not open. Maintainer decision (2026-09-25, #84): read it as an approximate real, as pdf.js does; the alternatives offered were to wrap while reading, as MuPDF and Acrobat do, or to saturate at `i64::MAX`/`MIN`. [Content stream parsing](content-stream-parsing.md) follows the same rule (`content/mod.rs`, "Beyond the i64 range").
- **Input bound:** a token is bounded only by the input slice. Nothing caps the length of a string, a name or a number. Recursion is not here: nesting depth belongs to the [Object model](object-model.md) parser.

## Code
- `justpdf-core/src/tokenizer/mod.rs` — `Tokenizer`, `Tokenizer::next_token`, `Tokenizer::seek`, `Tokenizer::new_at`, `read_literal_string`, `read_hex_string`, `read_name`, `read_number_or_keyword`, `read_keyword`, `classify_keyword`, `hex_val`, `test_integer_text_beyond_i64_reads_as_real`
- `justpdf-core/src/tokenizer/token.rs` — `Token`, `Keyword`, `Keyword::from_bytes`
- `justpdf-core/src/tokenizer/reader.rs` — `PdfReader`, `PdfReader::skip_whitespace_and_comments`, `is_pdf_whitespace`, `is_pdf_delimiter`, `is_pdf_regular`

## Reference behaviour
- Integer text beyond `i64`: pdf.js reads an approximate real; MuPDF and Acrobat wrap (#84, 2026-09-25, as the maintainer was shown it).
- Not compared otherwise. Clauses to compare against: ISO 32000-2 §7.2 (lexical conventions), §7.3.4 (strings), §7.3.5 (names). These clause numbers were not checked against the spec text.

## Cross-cutting invariants
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md) — every piece of syntax the write side produces must read back through this tokenizer as the same value.

## Blast radius
- [Object model](object-model.md) — `parse_object` reads `N M R` by reading ahead two tokens and seeking back. A change to what a token consumes, or to where `seek` lands, changes how references and arrays parse.
- [Object serialization](object-serialization.md) — string and name escape rules are a pair. A change to how a string or name is read must change how it is written, or the round trip breaks.
- [Content stream parsing](content-stream-parsing.md) — uses the same three byte-class functions. Changing which bytes are whitespace or delimiters changes content-stream lexing too.
- [Xref](xref.md), [Repair](repair.md), [Linearization](linearization.md), [Document access](document-access.md), [Object streams](object-streams.md) — they read file structure through this tokenizer, and some (xref tables, repair's header scan) also read bytes directly with their own rules. A change to whitespace or number rules here can make the two readings disagree.

## Known holes / open
**None.** No hole is known in the tokenizer itself. Unbounded token length is not a hole on its own: each token is bounded by the file in memory.
