# Content text encoding

## The fact
The bytes that go into a content stream's text-showing operators (`Tj`/`TJ`) are **character codes in the selected font's encoding**. Putting a Rust string's UTF-8 bytes in as they are turns every character outside ASCII into the wrong glyph with a standard Type1 font that has no encoding of its own (the WinAnsi family). Writing non-ASCII text correctly needs an embedded font that has the glyphs (a CID font, for example) and encoding the text in that font's codes.

## Why it is cross-cutting
Several features write text onto PDF pages, and each makes the same assumption — a standard Type1 font and UTF-8 bytes — **on its own**. The correct material ([CJK font embedding](../territory/cjk-font-embedding.md)) exists, but no writing path is connected to it. This is a different fact from [Text string encoding](text-string-encoding.md) — that one is about dictionary values (metadata), this one about the characters drawn on the page.

## Territories it holds in
- [Document builder](../territory/document-builder.md) — `PageBuilder::show_text` + `add_standard_font`. Most sites go through this pair.
- [Format document](../territory/format-document.md) — the common path for text formats (Courier 10pt).
- Per input format: [xps](../territory/xps-input.md), [epub](../territory/epub-input.md), [office](../territory/office-input.md), [mobi](../territory/mobi-input.md), [fb2](../territory/fb2-input.md), [plaintext](../territory/plaintext-input.md).
- [OCR](../territory/ocr.md) — the searchable text layer.
- Appearance generators: [Form appearance](../territory/form-appearance.md), [Signature appearance](../territory/signature-appearance.md), [Annotation appearance](../territory/annotation-appearance.md) (stamps).
- [CJK font embedding](../territory/cjk-font-embedding.md) — the material for the fix (not connected).

## What a violation looks like
- Running `justpdf convert` on Korean text in an EPUB, DOCX or text file prints fragments of Latin extended characters into the PDF (inferred — the repository has no non-ASCII conversion test).
- A visible signature with a Korean signer name and the appearance of a Korean form value come out broken.
- Text extraction has no ToUnicode, so it decodes as WinAnsi and returns a broken string as well.

## Discovery history
No incident recorded. On 2026-09-23, while the map was being written, four research agents (writing, interactive, signing, formats/special) reported it independently in their own areas. Every conclusion is inferred from reading code, and no test renders non-ASCII output to confirm it.

- Tracked: #34 (non-ASCII content text)

## Where it will recur
**A new feature that uses `show_text` or `Tj`, or a function that puts text into an appearance stream, is subject to this invariant.** Check: can the input be outside ASCII? If so, it needs an embedded font and that font's encoding instead of a standard font. Tests should use non-ASCII input and either render the result or read it back with ToUnicode-based extraction.
