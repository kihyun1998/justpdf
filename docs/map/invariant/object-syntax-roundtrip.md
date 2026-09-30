# Object syntax roundtrip

## The fact
Every byte justpdf writes as PDF syntax (objects, names, strings, numbers, content operators) must, when read back by justpdf's own parser ([Tokenizer](../territory/tokenizer.md), [Object model](../territory/object-model.md), [Content stream parsing](../territory/content-stream-parsing.md)), give **the same value as before writing**. Check shape: `parse(serialize(x)) == x`.

## Why it is cross-cutting
PDF syntax is not written in one place. The rules are in one place — `write_name`, `write_string` and `write_real` in `object/types.rs` (which `PdfObject` Display writes through), and `ContentOp::write_to`/`write_content`, which write content operators with them. But code that assembles operator text is spread across several modules, and each gets the rules only by calling these functions. The moment someone writes by hand, as in `write!(buf, "/{}", name)`, the rules drop out, and that defect is carried along no territory→territory edge.

## Territories it holds in
Central sites:
- [Object serialization](../territory/object-serialization.md) — `write_name`, `write_string`, `write_real`, Display, `serialize_object`, `serialize_dict`.
- [Content stream parsing](../territory/content-stream-parsing.md) — `ContentOp::write_to`, `Operand::write_to`, `write_content` (the only path that rewrites content operators, #29).
- [Incremental save](../territory/incremental-save.md) — writes appended objects and the trailer with `serialize_object`/`serialize_dict` (before #26 it wrote with Display, so streams broke into the debug format).
- [File serialization](../territory/file-serialization.md) — every object passes through `serialize_object`.
- [Tokenizer](../territory/tokenizer.md), [Object model](../territory/object-model.md) — the judges (the read-back side). CR normalization lives here. The judge on the content stream side is [Content stream parsing](../territory/content-stream-parsing.md), which does the same CR normalization (#87).

Sites that misuse Display: none. [Clean](../territory/clean.md) used Display text as object identity (dropping stream data, `Real(1.0)`=`Integer(1)`), and #27 switched it to value comparison — Display remains only as the bucket key for duplicate candidates.

Sites that write syntax by calling the shared functions (moved in #29):
- [Redaction](../territory/redaction.md), [Compress grayscale](../territory/compress-grayscale.md) — `write_content`/`ContentOp::write_to`.
- [Signing](../territory/signing.md) — `/Name`, `/Reason` and `/Location` through `string_syntax`, `/Rect` through `real_syntax`, the appearance stream dictionary through `serialize_dict`.
- [Signature appearance](../territory/signature-appearance.md), [Form appearance](../territory/form-appearance.md), [Annotation appearance](../territory/annotation-appearance.md) — `Tj` strings through `string_syntax`.
- [Document builder](../territory/document-builder.md) — names in `PageBuilder`'s `set_font`, `draw_image` and `draw_inline_image` through `name_syntax`, `show_text` through `write_string`.
- [Embedded files](../territory/embedded-files.md) — sets the MIME type as `Name(b"application/pdf")` and leaves it to the serializer.

Generator reals — appearance generators (form, annotation and signature appearances), `PageBuilder`'s coordinates, colors and font sizes, redaction's cover rectangles and colors — are written as `Number` (#90): an integer value within the 32-bit range is an integer (`612`), anything else goes through `write_real` (with a decimal point), NaN → `0`, ±Inf → ±`f32::MAX`. Normal output is the same as when it was written with `{}` — only NaN, inf, `-0` and integer values beyond 32 bits change.
- **Maintainer decision (2026-09-25, #90)**: the rule for generators. Alternative offered: the #28 rule as is (always a decimal point — `612` → `612.0`, changing all `DocumentBuilder` output). Facts offered: content stream operators do not distinguish integers from reals, readers with 32-bit integer parts (#85), MuPDF `fmt_obj` also writes integer values as integers. So the real rules for objects (`PdfObject` Display) and for generated content differ — the former is judged by value round-trip (`Real` to `Real`), the latter by unchanged output.
- Earlier decision (2026-09-25, #29 read-it): #29 excluded these sites — #90 is its follow-up.

Sites that rely on byte preservation (the half a tool cannot see — an assumption, not a call):
- [Object encryption](../territory/object-encryption.md), [Key derivation](../territory/key-derivation.md) — ciphertext, `/O` and `/U` must stay as they are even when they contain high bytes. #20 surfaced here.

A command to find the hand-writing sites again (read through the rest, filtering out test messages, examples and benches):
`rg -n '"/\{\}|/\{[a-z_]+\}|\(\{[a-z_]*\}\)|write!\(buf, "\{\}", obj\)|format!\("\{\}", obj\)' --glob '*.rs' --glob '!target' .`

## What a violation looks like
- A Name with a space, such as a Korean font name, breaks and font parsing fails → **Korean text disappears after a round trip** (0be6cc7).
- With certain password combinations only, an encrypted file **cannot be opened with any password** (#20). It reproduces only conditionally: only with passwords whose ciphertext bytes do not all take the literal path (0x20–0x7E). That is why common test passwords (`user123`/`owner456`) did not show it.
- `InvalidToken … invalid hex digit 0x73` when reading a stream back (the debug format `<stream …>`).
- The read-back value changes type (`Real(1.0)` → `Integer(1)`) or a CR becomes LF.

## Discovery history
The same fact was found and fixed three separate times:
1. 0be6cc7 — spaces in a Name not escaped as `#20` → Korean text lost. Regression tests `test_serialize_pdf_with_space_font_roundtrip`, `test_compress_font_name_with_space_roundtrip`, `test_name_display_with_space`.
2. 0be6cc7 — escaping parentheses and backslashes in a String.
3. 7be23c0 / #20 — high bytes (≥0x7F) in a String turned into UTF-8 multibyte sequences, damaging `/O` and `/U`. The issue first suspected key derivation; the cause was serialization.

All three times only the central site (Display) was fixed. The hand-writing sites did not get the same rules — the defects in the list above were found while writing the map on 2026-09-23, and of those, the `incremental_save` stream damage, the stream merging in `clean_objects`, and Display's CR, `Real(1.0)` and NaN round-trip holes were reproduced the same day with throwaway probes.

4. #28 — Display wrote a CR inside a literal raw (→ LF), wrote integer-valued reals without a decimal point (→ `Integer`, a parse error past i64), and wrote NaN and inf as they were. Encrypting a document whose source `/ID` held a CR made it openable by no one. This time a fixed case table, `test_written_objects_read_back_unchanged`, checks both the Display and `serialize_object` paths by reading them back, together with the three earlier defects (Name spaces, String parentheses, high bytes).

5. #29 — the hand-writing sites. Redaction (`format_operand`) wrote a string with unbalanced parentheses (`1)`) without escaping and changed the text, and both redaction and grayscale lost inline images (`<inline-image> BI`, `BI `). Names with spaces split into two operands, and large reals read back as different values (grayscale rounded every real to 4 decimal places). The embedded-file MIME type was double-escaped (`/application#232Fpdf`). The rules were pulled out into shared functions and every site made to call them. A fixed case table `test_written_ops_read_back_unchanged` (content operators) and per-site read-back tests were added.

- `incremental_save` switched to `serialize_object` in #26.

## Where it will recur
**Any function that produces PDF syntax bytes outside `PdfObject` Display / `serialize_object` is subject to this invariant.** When writing or changing one, check:
- Strings: does it emit bytes outside 0x20–0x7E and `\n\r\t` (especially ≥0x7F) as a literal? Then use hex. Does it write a raw CR into a literal? Then use `\r`.
- Reals: does it write integer values without a decimal point (→ reads back as `Integer`, and past i64 as `Real` — a parse error before #84)? Does it write NaN or inf as they are?
- Names: does it escape delimiters, whitespace, `#` and ≥0x7F as `#XX`? Does it avoid escaping an already-encoded value again?
- Streams: does it avoid writing them with `"{}"`?
- Where possible, do not write by hand: build a `PdfObject` and leave it to Display, or call `name_syntax`, `string_syntax`, `real_syntax` or `ContentOp::write_to`.
Tests must compare by **reading the written result back with the justpdf parser**. Assertions like `contains` or `starts_with(b"%PDF")` cannot check this invariant.
- The judge must read by the standard. Before #87 the content parser left a CR inside a literal as is, so even when the CR escape was missing at a content stream site the read-back value was the same — there was a rule the read-back tests could not see. When the judge shares the same model as the writer, reading back cannot see that model's defects.
