# Form field appearance generation

## What it is
Generates appearance streams for text, choice and button fields: in the AcroForm `/DR` font the field's `/DA` names, otherwise `/Helvetica 10 Tf`.

## Governing decisions
**None.**

## Design model
- **A field without `/DA`** (text, combo box, list box, push button) is drawn in justpdf's font: the text is `encode_winansi`d and the stream declares `/Resources /Font /Helvetica` as a WinAnsi Helvetica (measured, #34). `Tj` strings are written with `string_syntax` (#29) — [Content text encoding](../invariant/content-text-encoding.md), [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md). Coordinate reals are written as `Number` (#90).
- **A field with `/DA`** (its own or inherited from the AcroForm, `form::parse`) is drawn in the font the last `Tf` of `/DA` names, looked up in the AcroForm `/DR /Font` of the modifier's document, and the appearance's `/Resources /Font` declares it under that name with the `/DR` entry as it is (the same reference when `/DR` holds one). `{da}` is inserted verbatim. The text goes through `font::FontEncoder` ([Font encodings](font-encodings.md)) — maintainer decisions in #178's triage (2026-10-10): combo boxes, list boxes (every option shown) and push-button captions follow the same path as text fields; Type0 fonts are in scope; layout is #325's.
- **A value the `/DR` font cannot hold produces no appearance**: `generate_field_appearance` returns `AppearanceError::NotEncodable` and adds nothing to the modifier, so the viewer draws the field with its own fonts under `/NeedAppearances` (maintainer decision, #178 triage, 2026-10-10: no `?` substitution; a caller-supplied substitute is #175's). Also `NotEncodable`: `/DA` naming no font, a name missing from `/DR`, no `/DR`, and the fonts `FontEncoder::new` refuses. The other errors are `NoRect` (no `/Rect`, or no area) and `Signature`; until #178 all three were `None`.
- Every field that shows text writes `BT`, `0 g`, then the font selection (`{da}`, or `/Helvetica 10 Tf`), so the fill colour `/DA` sets is the one the text is drawn in and black is only the default. Until #340 a text field wrote `0 g` after `{da}` and drew `/DA (/Helv 12 Tf 1 0 0 rg)` black (measured on `ae3772a`); `every_field_draws_its_text_in_the_colour_da_sets`.
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
- There is no caller in product code (only its own tests and `tests/form_da_font.rs`).
- Tracked: #33 (text string encoding), #175 (text beyond WinAnsi through a caller-supplied appearance font), #325 (layout of variable text)
