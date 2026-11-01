# Text string encoding

## The fact
Human-readable PDF string values (Info Title and Author, outline titles, annotation contents, form values and field names, signer name and reason, page label prefixes, attachment file names and so on — the spec's "text string") must be **PDFDocEncoding or UTF-16BE with a BOM** (PDF 2.0 also allows UTF-8 with a BOM). Reading has to decode these encodings, and writing has to turn a Rust `&str` into one of them. UTF-8 bytes without a BOM are read by other readers as PDFDocEncoding and come out garbled.

## Why it is cross-cutting
Every document-level feature has its own place that reads and writes text strings, and there is no shared encoder. Correct decoders exist in two places (`decode_text` in [Font encodings](../territory/font-encodings.md), `obj_to_string` in [Embedded files](../territory/embedded-files.md)), but other modules do not reuse them, and **there is no UTF-16BE encoder anywhere in the repo** (re-checked 2026-10-30 at 88b2a08: `git grep -n 'encode_utf16\|0xFE, 0xFF' -- '*.rs'` hits only test inputs in `embedded_file.rs` and `font/encoding.rs`; measured). The sites do not call each other, so this fact can only be carried as a node, not as an edge. It is a different fact from [Object syntax roundtrip](object-syntax-roundtrip.md): that one is about preserving bytes, this one is about what the bytes mean — the bytes can round-trip perfectly and this invariant can still break.

## Territories it holds in
Writing (`&str` → UTF-8 without a BOM):
- [Document builder](../territory/document-builder.md) — Info, via `set_title` and the like.
- [Document modifier](../territory/document-modifier.md) — `set_info`.
- [Outlines](../territory/outlines.md) — titles.
- [Annotations](../territory/annotations.md) — `build_dict`.
- [Actions](../territory/actions.md) — `build_action`.
- [Embedded files](../territory/embedded-files.md) — `/F`, `/UF`.
- [Signing](../territory/signing.md) — `/Name`, `/Reason`, `/Location`, `/ContactInfo` and the field's `/T` (assembles the dictionary by hand, with `string_syntax`); `/T` is ASCII (`SignatureN`).
- [Page labels](../territory/page-labels.md) — prefix.
- [Optional content](../territory/optional-content.md) — OCG `/Name` written as raw UTF-8, read lossy (reported 2026-10-30; inferred).

Reading (`from_utf8_lossy`):
- [Annotations](../territory/annotations.md), [Outlines](../territory/outlines.md), [AcroForm](../territory/acroform.md) (`/T`, values), [Form appearance](../territory/form-appearance.md) (`value_as_string`), [Signature detection](../territory/signature-detection.md), [Page labels](../territory/page-labels.md).

Reading the same `/Info` values outside core, five more decoders that call neither core's nor each other (read 2026-10-30 at 88b2a08; inferred):
- [Facade](../territory/facade.md) — `Document::metadata_string`: UTF-16BE with a BOM, else Latin-1 (not PDFDocEncoding).
- [CLI](../territory/cli.md) — `print_info_field`: lossy UTF-8.
- [Python binding](../territory/python-binding.md), [Node binding](../territory/node-binding.md), [WASM binding](../territory/wasm-binding.md) — `get_info_string`: lossy UTF-8.

Where the correct decoders are:
- [Font encodings](../territory/font-encodings.md) — `decode_text`, `decode_utf16be`.
- [Embedded files](../territory/embedded-files.md) — `obj_to_string`.

The byte layer:
- [Object serialization](../territory/object-serialization.md) — preserves the bytes (hex path) but does not choose an encoding.

Command to find them again: the lines of `rg -n 'from_utf8_lossy|\.as_bytes\(\)\.to_vec\(\)' justpdf-core/src` that handle PDF string values. At 88b2a08 `from_utf8_lossy` appears in 30 files of `justpdf-core/src`, most of them names and diagnostics rather than text strings; the sites above are the text-string ones, and each listed symbol (`build_dict`, `build_action`, `set_info`, `set_title`, `value_as_string`, `decode_text`, `decode_utf16be`, `obj_to_string`) was found in its file (measured, 2026-10-30).

## What a violation looks like
- A justpdf → justpdf round trip is fine (UTF-8 written is read back as UTF-8), so justpdf's own tests cannot see it.
- In other viewers (Acrobat, browsers, MuPDF), Korean, Japanese and accented titles, bookmarks and signer names show up as mojibake.
- A UTF-16BE title made by another tool comes out full of U+FFFD when justpdf reads it.

## Discovery history
No incident was recorded. On 2026-09-23, while the map was being written, three research agents (writing, interactive features, signing) each reported the same defect in their own area, independently — 14 sites in one read. Research agent probe: `set_title("한글")` → `<ED959CEAB880>` (UTF-8 without a BOM).

- Tracked: #33 (text string encoding)

## Where it will recur
**Any function that puts human-readable text into a `PdfObject::String`, or takes human-readable text out of a `PdfObject::String`, is subject to this invariant.** Check: when writing can produce non-ASCII, does it encode as UTF-16BE + BOM; when reading, does it look at the BOM to decode. Until a shared encoder/decoder pair exists, every site will reimplement it.
