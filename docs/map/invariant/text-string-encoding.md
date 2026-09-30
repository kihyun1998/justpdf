# Text string encoding

## The fact
Human-readable PDF string values (Info Title and Author, outline titles, annotation contents, form values and field names, signer name and reason, page label prefixes, attachment file names and so on — the spec's "text string") must be **PDFDocEncoding or UTF-16BE with a BOM** (PDF 2.0 also allows UTF-8 with a BOM). Reading has to decode these encodings, and writing has to turn a Rust `&str` into one of them. UTF-8 bytes without a BOM are read by other readers as PDFDocEncoding and come out garbled.

## Why it is cross-cutting
Every document-level feature has its own place that reads and writes text strings, and there is no shared encoder. Correct decoders exist in two places (`decode_text` in [Font encodings](../territory/font-encodings.md), `obj_to_string` in [Embedded files](../territory/embedded-files.md)), but other modules do not reuse them, and **there is no UTF-16BE encoder anywhere in the repo**. The sites do not call each other, so this fact can only be carried as a node, not as an edge. It is a different fact from [Object syntax roundtrip](object-syntax-roundtrip.md): that one is about preserving bytes, this one is about what the bytes mean — the bytes can round-trip perfectly and this invariant can still break.

## Territories it holds in
Writing (`&str` → UTF-8 without a BOM):
- [Document builder](../territory/document-builder.md) — Info, via `set_title` and the like.
- [Document modifier](../territory/document-modifier.md) — `set_info`.
- [Outlines](../territory/outlines.md) — titles.
- [Annotations](../territory/annotations.md) — `build_dict`.
- [Actions](../territory/actions.md) — `build_action`.
- [Embedded files](../territory/embedded-files.md) — `/F`, `/UF`.
- [Signing](../territory/signing.md) — `/Name`, `/Reason`, `/Location` (assembles the dictionary by hand, with `string_syntax`).
- [Page labels](../territory/page-labels.md) — prefix.

Reading (`from_utf8_lossy`):
- [Annotations](../territory/annotations.md), [Outlines](../territory/outlines.md), [AcroForm](../territory/acroform.md) (`/T`, values), [Form appearance](../territory/form-appearance.md) (`value_as_string`), [Signature detection](../territory/signature-detection.md), [Page labels](../territory/page-labels.md).

Where the correct decoders are:
- [Font encodings](../territory/font-encodings.md) — `decode_text`, `decode_utf16be`.
- [Embedded files](../territory/embedded-files.md) — `obj_to_string`.

The byte layer:
- [Object serialization](../territory/object-serialization.md) — preserves the bytes (hex path) but does not choose an encoding.

Command to find them again: the lines of `rg -n 'from_utf8_lossy|\.as_bytes\(\)\.to_vec\(\)' justpdf-core/src` that handle PDF string values.

## What a violation looks like
- A justpdf → justpdf round trip is fine (UTF-8 written is read back as UTF-8), so justpdf's own tests cannot see it.
- In other viewers (Acrobat, browsers, MuPDF), Korean, Japanese and accented titles, bookmarks and signer names show up as mojibake.
- A UTF-16BE title made by another tool comes out full of U+FFFD when justpdf reads it.

## Discovery history
No incident was recorded. On 2026-09-23, while the map was being written, three research agents (writing, interactive features, signing) each reported the same defect in their own area, independently — 14 sites in one read. Research agent probe: `set_title("한글")` → `<ED959CEAB880>` (UTF-8 without a BOM).

- Tracked: #33 (text string encoding)

## Where it will recur
**Any function that puts human-readable text into a `PdfObject::String`, or takes human-readable text out of a `PdfObject::String`, is subject to this invariant.** Check: when writing can produce non-ASCII, does it encode as UTF-16BE + BOM; when reading, does it look at the BOM to decode. Until a shared encoder/decoder pair exists, every site will reimplement it.
