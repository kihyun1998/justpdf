# Form field appearance generation

## What it is
Generates appearance streams for text, choice and button fields: in the AcroForm `/DR` font the field's `/DA` names, otherwise `/Helvetica 10 Tf`.

## Governing decisions
**None.**

## Design model
- **A field without `/DA`** (text, combo box, list box, push button) is drawn in justpdf's font: the text is `encode_winansi`d and the stream declares `/Resources /Font /Helvetica` as a WinAnsi Helvetica (measured, #34). `Tj` strings are written with `string_syntax` (#29) — [Content text encoding](../invariant/content-text-encoding.md), [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md). Coordinate reals are written as `Number` (#90).
- **A field with `/DA`** (its own or inherited from the AcroForm, `form::parse`) is drawn in the font the last `Tf` of `/DA` names, looked up in the AcroForm `/DR /Font` of the modifier's document, and the appearance's `/Resources /Font` declares it under that name with the `/DR` entry as it is (the same reference when `/DR` holds one). `{da}` is inserted verbatim when it sets a size; an auto size (`0 Tf`) is rewritten to the size used (#325). The text goes through `font::FontEncoder` ([Font encodings](font-encodings.md)) — maintainer decisions in #178's triage (2026-10-10): combo boxes, list boxes (every option shown) and push-button captions follow the same path as text fields; Type0 fonts are in scope; layout is #325's.
- **A value the `/DR` font cannot hold produces no appearance**: `generate_field_appearance` returns `AppearanceError::NotEncodable` and adds nothing to the modifier, so the viewer draws the field with its own fonts under `/NeedAppearances` (maintainer decision, #178 triage, 2026-10-10: no `?` substitution; a caller-supplied substitute is #175's). Also `NotEncodable`: `/DA` naming no font, a name missing from `/DR`, no `/DR`, and the fonts `FontEncoder::new` refuses. The other errors are `NoRect` (no `/Rect`, or no area) and `Signature`; until #178 all three were `None`.
- Every field that shows text writes `BT`, `0 g`, then the font selection (`{da}`, or `/Helvetica 10 Tf`), so the fill colour `/DA` sets is the one the text is drawn in and black is only the default. Until #340 a text field wrote `0 g` after `{da}` and drew `/DA (/Helv 12 Tf 1 0 0 rg)` black (measured on `ae3772a`); `every_field_draws_its_text_in_the_colour_da_sets`.
- **Background and border come from the widget's `/MK` and `/BS`** (`WidgetLook`, #333), read from the first of `FormField::widgets` in the modifier. Form fill passes a copy whose `widgets` is the one widget it redraws (#344's `redraw_widgets`).
  - Rules:
    - `/MK /BG` is filled as `0 0 w h re f`.
    - `/MK /BC` is stroked at border width `b` as `b/2 b/2 w-b h-b re S`.
    - `b` is `/BS /W`, else `/Border[2]`, else 1. A border with `b = 0` is not drawn.
    - A colour array of 1, 3 or 4 components is written as `g`/`rg`/`k` (fill) or `G`/`RG`/`K` (stroke). Any other length, 0 included, is no colour.
    - Without `BG` there is no background, and without `BC` no border.
    - A radio button fills and strokes its circle of radius `min(w, h)/2 − b/2`.
    - The stroke is solid whatever `/BS /S` says.
  - These rules come from MuPDF `pdf_write_tx_widget_appearance` (`pdf-appearance.c`) and pdf.js `getBorderAndBackgroundAppearances` (raw source, read 2026-10-23, #333 triage). Both draw nothing without `/MK` and ignore the style.
  - Maintainer decisions (#333 triage, 2026-10-23):
    - no `/MK` means no background or border
    - only solid borders
    - every field type follows `/MK`, the push button included (its bevelled look went; decided when #333 was read)
  - The `b/2` inset and the width fallback are derivations from MuPDF.
  - Until #333 every type drew a fixed look: a text field `0.75 g` with a black 0.5 pt border, other fields `1 g`, a push button a `0.85 g` bevel. It ignored `/MK` (measured).
  - What stays per type, drawn after the background and border: the checkmark, the radio dot, the combo box's arrow area, the list box's highlight and text.
  - `WidgetLook::width` is the border width the text box is computed from (#325).
- **A text field or combo box lays its value out on one line** (`single_line`, #325). The constants and formulas follow pdf.js `TextWidgetAnnotation` (`_getAppearance`, `_computeFontSize`, `_renderText`; `LINE_FACTOR` 1.35 and `LINE_DESCENT_FACTOR` 0.35 from `src/shared/util.js`), read from raw source on 2026-10-23.
  - **Text box** (GLOSSARY): the widget rectangle inset by the border width `b`, `[b, b, w − 2b, h − 2b]`. A combo box also leaves out its arrow area, `h.min(20)` wide. The value is clipped to it, as MuPDF clips (`pdf_write_tx_widget_appearance`); pdf.js does not clip.
  - **Size:** the `/DA` size when non-zero (10 without `/DA`). With `0 Tf` it is `floor₂(min((bh − 2)/1.35, (bw − 4)/em))`, where `em` is the value's width at size 1. No value is drawn when the box is no wider than 4 or no taller than 2. A fixed size that overflows is clipped, not shrunk.
  - **x:** `/Q` 0 puts it at `x0 + 2`, 1 at `x0 + (bw − tw)/2`, 2 at `x0 + bw − tw − 2`.
  - **y:** the baseline is at `y0 + (bh − (asc + desc)·size)/2 + desc·size`.
  - **`/Q`:** the widget's own `/Q` (0–2), else `FormField::quadding`: the field's, its ancestors', the AcroForm's, else 0. pdf.js reads it the same way (`getInheritableProperty` on the widget); MuPDF reads only the widget's. Maintainer decision (#325, 2026-10-23): widget first.
  - **Ascent and descent:** from the font descriptor unless both are 0, else the Standard 14 AFM values. Measured from the Adobe AFMs in Apache PDFBox: Helvetica 718/207, Times 683/217, Courier 629/157; Symbol and ZapfDingbats have none. Otherwise pdf.js's fallback, 1.0/0.35 em (`font::FontEncoder::ascent_descent`, `standard14_ascent_descent`).
  - **Widths:** the `/DR` font's `/Widths` (or the Standard 14 table) by code, a Type0 font's `/W` by CID. Without `/DA`, the WinAnsi Helvetica table.
  - **Structure:** `/Tx BMC q`, background and border, the clip `re W n`, `BT 0 g {da} x y Td (…) Tj ET`, then `Q EMC`. The background and border are **inside** the marked content, as in MuPDF and pdf.js. Maintainer decision (#325, 2026-10-23): this replaces #325's triage call to put them outside, which had been made without reading either reference. A password field, or one with an empty value, gives `/Tx BMC q {decoration} Q EMC`.
  - Until #325 every value was drawn at `2 (h − 10)/2 Td`, and `0 Tf` drew it invisibly (inferred).
- Values go through `value_as_string` (`from_utf8_lossy`), so a UTF-16BE value becomes U+FFFD.
- Signature fields return `None`.

## Code
- `justpdf-core/src/form/appearance.rs` — `generate_field_appearance`, `WidgetLook`, `colour_operator`, `radio_appearance`

## Reference behaviour
MuPDF `source/pdf/pdf-appearance.c` and pdf.js `src/core/annotation.js` for `/MK` and `/BS` (#333). Clause to compare against: ISO 32000-2 §12.7.4.3 (variable text), §12.5.6.19 (`/MK`).

## Cross-cutting invariants
- [Content text encoding](../invariant/content-text-encoding.md)
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md)
- [Text string encoding](../invariant/text-string-encoding.md) — decoding field values.

## Blast radius
- [Form fill](form-fill.md) — calls this generator for each widget of a text field, combo box or list box it fills (#344).
- [Form flatten](form-flatten.md) — the side that puts appearances into the page.
- [Annotation appearance](annotation-appearance.md), [Signature appearance](signature-appearance.md) — the two other generators.

## Known holes / open
- Tracked: #33 (text string encoding), #175 (text beyond WinAnsi through a caller-supplied appearance font), #332 (multiline and comb), #292 (approximate Standard 14 widths), `/BS /S` styles other than solid and `/MK /CA`, `/R`, `/I` (not tracked)
