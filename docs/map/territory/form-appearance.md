# Form field appearance generation

## What it is
Generates appearance streams for text, choice and button fields (the `/DA` font if there is one, otherwise `/Helvetica 10 Tf`).

## Governing decisions
**None.**

## Design model
- Where justpdf picks the font — a text field without `/DA`, and every combo box, list box and push button (`uses_default_font`) — the text is `encode_winansi`d and the stream declares `/Resources /Font /Helvetica` as a WinAnsi Helvetica (measured, #34). A text field with `/DA` keeps the UTF-8 bytes and declares nothing: its font comes from `/DR`, whose encoding is not justpdf's to choose (#173). `Tj` strings are written with `string_syntax` (#29) — [Content text encoding](../invariant/content-text-encoding.md), [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md). Coordinate reals are written as `Number` (#90).
- It does not link `/DR`. `{da}` is inserted verbatim.
- Values go through `value_as_string` (`from_utf8_lossy`), so a UTF-16BE value becomes U+FFFD.
- Signature fields return `None`.

## Code
- `justpdf-core/src/form/appearance.rs` — `generate_field_appearance`, `radio_appearance`

## Reference behaviour
**None.** Clause to compare against: ISO 32000-2 §12.7.4.3 (variable text).

## Cross-cutting invariants
- [Content text encoding](../invariant/content-text-encoding.md)
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md)
- [Text string encoding](../invariant/text-string-encoding.md) — decoding field values.

## Blast radius
- [Form fill](form-fill.md) — the side that should call this generator (it does not today).
- [Form flatten](form-flatten.md) — the side that puts appearances into the page.
- [Annotation appearance](annotation-appearance.md), [Signature appearance](signature-appearance.md) — the two other generators.

## Known holes / open
- There is no caller in product code (only its own tests).
- Tracked: #33 (text string encoding), #173 (fonts for `/DA` fields and text beyond WinAnsi)
