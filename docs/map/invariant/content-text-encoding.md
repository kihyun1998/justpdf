# Content text encoding

## The fact
The bytes that go into a content stream's text-showing operators (`Tj`/`TJ`) are **character codes in the selected font's encoding**. Putting a Rust string's UTF-8 bytes in as they are turns every character outside ASCII into the wrong glyph — plain Latin such as `é`, `€` and `—` too, not only CJK (measured before #34: `show_text("café — €")` extracted as `cafˆ' ‹`).

Where justpdf chooses the font itself, the rule since #34 is: the font is a standard Type1 font declared `/Encoding /WinAnsiEncoding` (except `Symbol` and `ZapfDingbats`, which keep their built-in encoding — `writer::document::standard_font_dict`), and the text goes through `font::encode_winansi` (next to the WinAnsi decode table in `font/encoding.rs`): ASCII as it is, any other character its Windows-1252 code, `?` when it has none — including the five codes WinAnsi leaves without a glyph (0x81, 0x8D, 0x8F, 0x90, 0x9D). Characters outside WinAnsi need an embedded font that has the glyphs and that font's codes — #173.

## Why it is cross-cutting
Several features write text onto PDF pages, and each made the same assumption — a standard Type1 font and UTF-8 bytes — **on its own**. Since #34 they share one encoder and one standard-font dictionary. The material for text beyond WinAnsi ([CJK font embedding](../territory/cjk-font-embedding.md)) exists, but no writing path is connected to it (#173). This is a different fact from [Text string encoding](text-string-encoding.md) — that one is about dictionary values (metadata), this one about the characters drawn on the page.

## Territories it holds in
- [Document builder](../territory/document-builder.md) — `PageBuilder::show_text` + `add_standard_font` / `add_font`. Most sites go through this pair.
- [Format document](../territory/format-document.md) — the common path for text formats (Courier 10pt).
- Per input format: [xps](../territory/xps-input.md), [epub](../territory/epub-input.md), [office](../territory/office-input.md), [mobi](../territory/mobi-input.md), [fb2](../territory/fb2-input.md), [plaintext](../territory/plaintext-input.md).
- [OCR](../territory/ocr.md) — the searchable text layer.
- Appearance generators: [Form appearance](../territory/form-appearance.md), [Signature appearance](../territory/signature-appearance.md), [Annotation appearance](../territory/annotation-appearance.md) (stamps).
- [Font encodings](../territory/font-encodings.md) — owns the WinAnsi table and `encode_winansi`.
- [CJK font embedding](../territory/cjk-font-embedding.md) — the material for text beyond WinAnsi (not connected, #173).

## What a violation looks like
- A path that writes `text.as_bytes()` into `Tj` instead of `encode_winansi`, or selects a font it does not declare: Latin text beyond ASCII comes out as other glyphs, and extraction returns a broken string.
- Since #34, Korean (or any text outside WinAnsi) comes out as `?` in converters, the OCR layer and appearances — the defined substitution, not a violation (measured: `show_text("café 한글")` extracts as `café ??`).

## Discovery history
No incident recorded. On 2026-09-23, while the map was being written, four research agents (writing, interactive, signing, formats/special) reported it independently in their own areas. Every conclusion is inferred from reading code, and no test renders non-ASCII output to confirm it.

- 2026-10-07, #34: measured before the fix — `show_text("café — €")` with `add_standard_font("Helvetica")` extracted as `cafˆ' ‹`, `PlainTextDocument` and DOCX conversions likewise; the plaintext, MOBI and FB2 wrappers panicked on a line whose cut fell inside a multi-byte character (`byte index 78 is not a char boundary`). Resolved for the WinAnsi slice; tests in `writer/document.rs`, `writer/page.rs`, the three appearance modules, and each converter.
- Tracked: #173 (text beyond WinAnsi through an embedded font)

## Where it will recur
**A new feature that uses `show_text` or `Tj`, or a function that puts text into an appearance stream, is subject to this invariant.** Check: can the input be outside ASCII? If so, it needs an embedded font and that font's encoding instead of a standard font. Tests should use non-ASCII input and either render the result or read it back with ToUnicode-based extraction.
