# Content text encoding

## The fact
The bytes that go into a content stream's text-showing operators (`Tj`/`TJ`) are **character codes in the selected font's encoding**. Putting a Rust string's UTF-8 bytes in as they are turns every character outside ASCII into the wrong glyph — plain Latin such as `é`, `€` and `—` too, not only CJK (measured before #34: `show_text("café — €")` extracted as `cafˆ' ‹`).

Where justpdf chooses the font itself, the rule since #34 is: the font is a standard Type1 font declared `/Encoding /WinAnsiEncoding` (except `Symbol` and `ZapfDingbats`, which keep their built-in encoding — `writer::document::standard_font_dict`), and the text goes through `font::encode_winansi` (next to the WinAnsi decode table in `font/encoding.rs`): ASCII as it is, any other character its Windows-1252 code, `?` when it has none — including the five codes WinAnsi leaves without a glyph (0x81, 0x8D, 0x8F, 0x90, 0x9D). Characters outside WinAnsi need an embedded font that has the glyphs and that font's codes: since #173 a caller-supplied TrueType program registered as a `Font` (`add_font`) is written as a Type0 Identity-H font, and `show_text` writes 2-byte CIDs for it, with fallback fonts per character — [CJK font embedding](../territory/cjk-font-embedding.md). It maps characters one-to-one with no shaping, bidi or vertical writing (#177). The converters, the OCR layer and the appearance generators still choose a standard font and so still write `?` beyond WinAnsi. Tracked: #174, #175, #322 (re-read 2026-10-30 at 88b2a08: `encode_winansi` in `font/encoding.rs` and `standard_font_dict` in `writer/document.rs` are where this note says; inferred).

## Why it is cross-cutting
Several features write text onto PDF pages, and each made the same assumption — a standard Type1 font and UTF-8 bytes — **on its own**. Since #34 they share one encoder and one standard-font dictionary. Since #173 `PageBuilder` draws text beyond WinAnsi through an embedded `Font` ([CJK font embedding](../territory/cjk-font-embedding.md)); the paths where justpdf chooses the font have not moved to it yet (#174, #175, #322). This is a different fact from [Text string encoding](text-string-encoding.md) — that one is about dictionary values (metadata), this one about the characters drawn on the page.

## Territories it holds in
- [Document builder](../territory/document-builder.md) — `PageBuilder::show_text` + `add_standard_font` / `add_font`. Most sites go through this pair.
- [Format document](../territory/format-document.md) — the common path for text formats (Courier 10pt).
- Per input format: [xps](../territory/xps-input.md), [epub](../territory/epub-input.md), [office](../territory/office-input.md), [mobi](../territory/mobi-input.md), [fb2](../territory/fb2-input.md), [plaintext](../territory/plaintext-input.md).
- [OCR](../territory/ocr.md) — the searchable text layer.
- Appearance generators: [Form appearance](../territory/form-appearance.md), [Signature appearance](../territory/signature-appearance.md), [Annotation appearance](../territory/annotation-appearance.md) (stamps).
- [Font encodings](../territory/font-encodings.md) — owns the WinAnsi table and `encode_winansi`.
- [CJK font embedding](../territory/cjk-font-embedding.md) — `Font`, `FontProgram` and the Type0 write path for text beyond WinAnsi.

## What a violation looks like
- A path that writes `text.as_bytes()` into `Tj` instead of `encode_winansi`, or selects a font it does not declare: Latin text beyond ASCII comes out as other glyphs, and extraction returns a broken string.
- Since #34, Korean (or any text outside WinAnsi) comes out as `?` in converters, the OCR layer and appearances — the defined substitution, not a violation (measured: `show_text("café 한글")` extracts as `café ??`).

## Discovery history
No incident recorded. On 2026-09-23, while the map was being written, four research agents (writing, interactive, signing, formats/special) reported it independently in their own areas. Every conclusion is inferred from reading code, and no test renders non-ASCII output to confirm it.

- 2026-10-07, #34: measured before the fix — `show_text("café — €")` with `add_standard_font("Helvetica")` extracted as `cafˆ' ‹`, `PlainTextDocument` and DOCX conversions likewise; the plaintext, MOBI and FB2 wrappers panicked on a line whose cut fell inside a multi-byte character (`byte index 78 is not a char boundary`). Resolved for the WinAnsi slice; tests in `writer/document.rs`, `writer/page.rs`, the three appearance modules, and each converter.
- 2026-10-22, #173: measured before the change — `show_text("café 한글")` with `add_standard_font("Helvetica")` wrote `<636166C3A920ED959CEAB880> Tj` before #34. After it, a Hangul fixture with Noto Sans as fallback draws `café 한글 😀` and extracts it back exactly through justpdf's extractor; the emoji is `.notdef` and in `missing_chars()`, and justpdf-render draws the Hangul as glyphs, not `.notdef` (tests in `justpdf-core/tests/unicode_font.rs` and `justpdf-render/tests/render_embedded_font.rs`).
- Tracked: #174 (converters), #175 (appearances), #322 (OCR) — still standard fonts.

## Where it will recur
**A new feature that uses `show_text` or `Tj`, or a function that puts text into an appearance stream, is subject to this invariant.** Check: can the input be outside ASCII? If so, it needs an embedded font and that font's encoding instead of a standard font. Tests should use non-ASCII input and either render the result or read it back with ToUnicode-based extraction.
