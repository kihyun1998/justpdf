# Signature appearance (visible signature)

## What it is
`generate_signature_appearance` builds the Form XObject drawn for a visible signature: a grey-bordered light box the size of the widget, and up to three lines of 10 pt Helvetica — "Digitally signed by: <name>", "Reason: <reason>", "Date: <date>". [Signing](signing.md) calls it for `SigningOptions::visible`.

## Governing decisions
**None.** The glossary's **appearance font** (`GLOSSARY.md`, "Appearances") names the caller-supplied font this generator is to draw with; that is decided on the tracker (#175, from #173's grilling) and not built.

## Design model
From reading the source (`sign/appearance.rs`) and its unit tests.
- **Font**: each line is `encode_winansi` of the text (a character with no WinAnsi code becomes `?`), written into `Tj` with `string_syntax` (#29), in `/F1`, which the stream's own `/Resources` declares as Type1 Helvetica with `/WinAnsiEncoding` (`standard_font_dict`). `Müller` reads back as the bytes `M\xFCller` (`test_text_is_win_ansi_in_a_declared_win_ansi_font`, #34); in the stream that string is a hex string, since `string_syntax` writes any byte ≥ 0x7F as hex — [Content text encoding](../invariant/content-text-encoding.md), [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md).
- **Layout**: the first line starts 4 pt in from the left and 14 pt below the top; each further line 12 pt lower. Text is neither wrapped nor clipped to the box, so a long name runs past the border (inferred). Width and height are written with `Number` and `/BBox` as reals; non-finite sizes are written as numbers (`test_non_finite_sizes_are_written_as_numbers`).
- **Date**: `build_pdf_with_placeholder` passes `date = None`, so no date line is drawn.
- The stream is Flate-compressed (`make_stream(…, true)`); `/Matrix` is not written.
- **Bounds on input**: name and reason lengths are not bounded.

## Code
- `justpdf-core/src/sign/appearance.rs` — `generate_signature_appearance`
- `justpdf-core/src/writer/document.rs` — `standard_font_dict`
- `justpdf-core/src/font/encoding.rs` — `encode_winansi`

## Reference behaviour
**None.** MuPDF's appearance options (labels, DN, date, logo) are listed in `docs/mupdf-feature-analysis.md` §3.4, not compared.

## Cross-cutting invariants
- [Content text encoding](../invariant/content-text-encoding.md) — WinAnsi bytes in a font declared WinAnsi.
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md) — `string_syntax`.

## Blast radius
- [Signing](signing.md) — the caller; it decides size, name, reason and the missing date.
- [Form appearance](form-appearance.md), [Annotation appearance](annotation-appearance.md) — the other two generators that pick their own font; all three write WinAnsi Helvetica through `string_syntax`, and #175 is to give all three the same caller-supplied font, so a font rule changed in one is checked in the others.

## Known holes / open
- Text beyond WinAnsi draws as `?`; drawing it needs a caller-supplied appearance font. Tracked: #175.
- Long text overflows the box (inferred). Not tracked by an issue.
- No date line, though the generator supports one (above).
