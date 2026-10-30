# Text wrapping

## What it is
`text::text_layout` wraps text to a width, aligns it and measures it, from a reading-side `FontInfo`'s widths. It lives under `text/` beside extraction but belongs to generation. It is public API (`justpdf_core::text::text_layout`) with no caller: the writing paths that wrap text each do it their own way.

## Governing decisions
**None.** No decision record. #174's brief records the maintainer's #173 grilling (2026-10-02): one shared text-page layout in `justpdf-formats`, wrapping by the measured advance of the selected `Font`. That layout is planned, and it is not this module.

## Design model
Status: from reading the source; no record.

- `layout_text(text, font, options)` splits paragraphs at newlines, wraps each at word boundaries to `max_width` (`wrap_paragraph`), breaks a word longer than a line character by character (`force_break_word`), applies `first_line_indent`, places lines by `line_height_factor` × `font_size` and `alignment`, and stops at `max_height`, setting `overflow` (inferred, read from `layout_text`).
- **Widths take the Unicode scalar as the character code**: `char_width(ch, font)` is `font.widths.get_width(ch as u32)`. That is right only where code and scalar agree (ASCII in WinAnsi or Standard); for any other character, or a font whose encoding is not Identity-to-Unicode, the width is another glyph's or the default (inferred).
- No kerning ([OpenType layout](opentype-layout.md) is not used).
- **No caller.** `git grep 'layout_text\|measure_text_width\|text_layout'` outside the file finds only `pub mod text_layout` in `text/mod.rs` (measured 2026-10-30).
- **Parallel implementations** it does not serve: `Font::advance` measures text for a `Font` handle ([CJK font embedding](cjk-font-embedding.md)); form appearances wrap multiline fields in `multiline_text` with `FontEncoder` widths ([Form appearance](form-appearance.md)); plaintext, fb2 and mobi wrap by character count at `font_size * 0.6` (`wrap_fixed_pitch`, fb2 `wrap_text`; [Format document](format-document.md)) (inferred, `git grep`).
- **Input bound.** The text comes from the caller; nothing bounds its length (inferred).

## Code
- `justpdf-core/src/text/text_layout.rs` — `layout_text`, `measure_text_width`, `char_width`, `wrap_paragraph`, `force_break_word`, `LayoutOptions`, `LayoutResult`, `LayoutLine`, `TextAlignment`

## Reference behaviour
**None.** Not compared against a reference.

## Cross-cutting invariants
**None.**

## Blast radius
- [Document builder](document-builder.md) — the side that would use wrapping; it would need widths by the code written, not by Unicode scalar.
- [Format document](format-document.md) — plaintext, mobi and fb2 share `common::wrap_fixed_pitch` (fb2 also has its own `wrap_text`), and epub, office and xps break lines in their own loops; #174's shared layout would replace those loops, and this module, if reused, would have to measure through `Font` rather than `FontInfo`.
- [Form appearance](form-appearance.md) — `multiline_text` is a second word wrapper; a behaviour change in one does not reach the other.

## Known holes / open
- No caller in the workspace (measured, `git grep`).
- Widths are wrong for any character whose code is not its Unicode scalar (inferred).
- Text converters have no shared width-based layout and take no caller font. Tracked: #174.
- Open question for the maintainer: keep `text::text_layout` as public API, rebuild it on `Font` for #174, or remove it?
