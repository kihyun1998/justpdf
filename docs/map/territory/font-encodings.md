# Font encodings (simple fonts)

## What it is
The encoding tables (Standard, WinAnsi, MacRoman, PDFDoc, Identity) that turn a simple font's 1-byte codes into Unicode, and the interpretation of `/Encoding`. It is the path text extraction uses when there is no ToUnicode, and the decoder for PDF text strings (PDFDocEncoding / UTF-16BE BOM) also lives here.

## Governing decisions
**None.**

## Design model
- **`/Differences` is not handled**: an encoding dictionary falls back to `StandardEncoding` (the TODO in `parse_encoding`). The only `/Differences` parser is in the Type3-only `parse_type3_font`, and nothing calls that function.
- MacRoman uses the WinAnsi table ("simplified — uses same table for now"). StandardEncoding is also decoded as WinAnsi ("close enough for display").
- The renderer does not use these encodings: `char_code_to_glyph_id` treats the byte as a Unicode scalar and looks it up in the font's cmap.
- The function that decodes PDF text strings correctly (`decode_text`, handling the UTF-16BE BOM) is here, but the annotation, outline, form and signature readers do not use it; they use `from_utf8_lossy` — [Text string encoding](../invariant/text-string-encoding.md).

## Code
- `justpdf-core/src/font/encoding.rs` — `Encoding`, `from_name`, `decode_text`, `decode_winansi`, `decode_mac_roman`, `decode_pdfdoc`, `decode_utf16be`
- `justpdf-core/src/font/mod.rs` — `parse_encoding`

## Reference behaviour
**None.** To compare against: ISO 32000-2 Annex D (character sets and encodings), §9.6.5.

## Cross-cutting invariants
- [Text string encoding](../invariant/text-string-encoding.md) — where the only correct decoder lives.
- [Font resolution](../invariant/font-resolution.md) — text uses these tables, render uses the font's cmap.

## Blast radius
- [Text extraction](text-extraction.md) — `show_string` calls `decode_text`.
- [Glyph rendering](glyph-rendering.md) — the side that does not use the encodings. Supporting `/Differences` means adding it to render separately too.
- [Type3 fonts](type3-fonts.md) — where the `/Differences` parser is.
- [ToUnicode](tounicode.md) — the higher-priority mapping.

## Known holes / open
- Not handling `/Differences` makes text extraction wrong for fonts with non-standard encodings: Helvetica with `/Differences [65 /B]` showing `(A)` extracts `"A"`, and `/MacRomanEncoding` `<8E>` extracts `"Ž"` instead of `"é"` (measured 2026-10-06, synthetic pages, during #45's triage). Such fonts are among the most common in real-world files (inferred).
- Tracked: #33 (text string encoding), #221 (/Differences and base tables in text), #223 (encoding in render glyph selection, and the shared §9.6.6.4 code → GID function; blocked by #221)
