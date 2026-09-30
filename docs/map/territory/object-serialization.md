# Object serialization

## What it is
The side that turns `PdfObject` values into PDF syntax bytes. `impl Display for PdfObject` writes every object except streams, and `serialize_object` handles streams alone directly (setting `/Length` to `data.len()`). It is the bottom layer of every path that produces a file — creation, modification, compression, encryption, signing.

## Governing decisions
**None.**

## Design model
- **The rules live in one place**: `write_name`, `write_string` and `write_real` (writing to `fmt::Write`) are the rules, and reals in generated content are written as `Number` (integer values as integers — the generator reals of [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md), #90). Display, `serialize_dict` and content operator writing (`ContentOp::write_to`) call these. Into a byte buffer they write through `ByteSink`, and where a fragment string is needed through `name_syntax`, `string_syntax` and `real_syntax` (before #29 there were three copies of the name rule — Display `Name`, Display `Dict` keys, `write_escaped_name`).
- **Names**: delimiter, whitespace, `#`, `<= 0x20` and `>= 0x7F` bytes are escaped as `#XX`. `serialize_dict` is a second copy of Display `Dict` (the only difference is writing stream values with `serialize_object`).
- **Strings**: literal `(…)` only when every byte is 0x20–0x7E or `\n\r\t`, otherwise hex. In a literal, `(`, `)` and `\` are escaped and CR is written as `\r` (#28). [Tokenizer](tokenizer.md) turning line breaks (CR, CRLF) inside a literal into LF is behaviour ISO 32000 §7.3.4.2 requires, so a raw CR reads back as LF — before #28, encrypting a document whose source `/ID` first element held a CR changed the read-back `/ID[0]`, so the R3/R4 file key went wrong and neither justpdf nor qpdf could open it. The hex rule exists because `b as char` on the literal path turned high bytes into UTF-8 multibyte sequences and broke binary strings such as /O and /U (a comment records this reason).
- **Reals**: always written with a decimal point — `1.0`, `-0.0`, `100000000000000000000.0` even for integer values (#28). Reals that are not integer values are Rust `f64` Display as is (shortest round-trip digits, no exponent notation). NaN is written as `0.0` and ±Inf as ±`f32::MAX` — PDF cannot express them and Display cannot fail. NaN only arises from computation, but Inf also comes in from files — the tokenizer reads real text as `f64`, so a real beyond the `f64` range (about 1.8e308; `1` followed by 400 zeros and `.0`, say) becomes `inf` and is written back as `f32::MAX` (assay and lens probes, 2026-09-25).
  - Before #28: `Real(1.0)` → `1` → `Integer(1)`, `Real(-0.0)` → `Integer(0)`, `Real(1e20)` → integer text past i64, so **reading back was a parse error**, NaN and inf → `NaN` and `inf` (not valid PDF).
  - **Maintainer decision (2026-09-25, #28 triage)**: always a decimal point, and NaN and Inf are written as substitutes. Alternatives offered: write integer values within the i64 range as integers as MuPDF does (harmless under ISO 32000 §7.3.3, but it reads back as `Integer`, a different value under value comparison); make NaN and Inf a `serialize_object` error. Facts offered as grounds: the reproduction above, MuPDF `pdf-object.c` `fmt_obj` (integer-valued reals as `%d`) and `printf.c` `fmtfloat` ("NaN to 0, +Inf to FLT_MAX, -Inf to -FLT_MAX").
- **Streams**: Display prints a stream in a debug format that is not PDF (`<stream dict=… len=N>`). Streams must be written with `serialize_object`; writing them with `"{}"` breaks the file.

## Code
- `justpdf-core/src/object/types.rs` — `PdfObject`, `write_name`, `write_string`, `write_real`, `Number`, `name_syntax`, `string_syntax`, `real_syntax`, `ByteSink`, `test_string_display_high_byte_uses_hex`, `test_name_display_with_space`
- `justpdf-core/src/writer/serialize.rs` — `serialize_object`, `serialize_dict`, `test_written_objects_read_back_unchanged`, `test_non_finite_reals_are_written_as_finite_ones`, `test_serialize_pdf_with_space_font_roundtrip`, `test_serialize_string_with_parens_roundtrip`
- `justpdf-core/src/writer/modify.rs` — `test_build_with_encryption_keeps_a_permanent_id_that_holds_a_carriage_return`

## Reference behaviour
MuPDF `source/pdf/pdf-object.c` `fmt_obj` (writes integer-valued reals as integers — justpdf goes the other way here, see the decision above), `source/fitz/printf.c` `fmtfloat` (NaN and Inf handling — justpdf goes the same way). Clauses to compare against: ISO 32000-2 §7.3.4 (strings), §7.3.5 (names), §7.3.3 (numbers). The check against the spec text (2026-09-25) was done with ISO 32000-1 §7.3.4.2 (a line break inside a literal reads as 0Ah) and §7.3.3 (reals with a decimal point, no exponent notation — NOTE 1; an integer is allowed where a real is expected — NOTE 2).

## Cross-cutting invariants
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md) — this note is the invariant's central site.
- [Text string encoding](../invariant/text-string-encoding.md) — serialization only preserves bytes; there is no encoder at this layer that turns a Rust `&str` into a PDF text string.

## Blast radius
- [Tokenizer](tokenizer.md), [Object model](object-model.md) — when changing a write rule, check that the read side reads it back unchanged.
- [File serialization](file-serialization.md), [Incremental save](incremental-save.md), [Clean](clean.md) — callers that use Display directly. Especially places that write a stream with `"{}"`.
- [Object encryption](object-encryption.md) — relies on ciphertext strings taking the hex path.
- [Compress](compress.md) — every compressed output file goes through this serialization.

## Known holes / open
- Streams nested inside a dictionary or array are printed in the debug format.
- Out-of-range reals: finite values beyond the f32 range (1e300 → 301 digits) and very small values (5e-324 → 326 characters) are written as they are (ISO 32000-1 Annex C.2 recommends staying within ±3.403e38). MuPDF and Acrobat read reals with an integer part of 10 or more digits truncated to 32 bits, so `f32::MAX` written for ±Inf and `1e20` also become different values (MuPDF `pdf-lex.c` `acrobat_compatible_atof`, computed). The ±Inf→±`f32::MAX` decision was made without seeing this, and the maintainer chose to leave #28 as it was and decide this separately (2026-09-25). Tracked: #85
