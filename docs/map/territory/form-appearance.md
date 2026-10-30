# Form field appearance generation

## What it is
Generates a normal appearance stream for a form field widget (`generate_field_appearance`): text fields, checkboxes, radio buttons, combo boxes, list boxes and push buttons, drawn in the AcroForm `/DR` font that the field's `/DA` names, or in justpdf's WinAnsi `/Helvetica` when there is no `/DA`. Signature fields get none.

## Governing decisions
**None.** The rules below are maintainer decisions recorded on issues (#178, #325, #332, #333), and pdf.js and MuPDF behaviour read from their source. The glossary's **Text box** and **Appearance font** name terms used here.

## Design model
### Font and text
- **A field without `/DA`** (text, combo box, list box, push button) is drawn in justpdf's font: the text is `encode_winansi`d and the stream declares `/Resources /Font /Helvetica` as a WinAnsi Helvetica (#34; `default_font_fields_write_win_ansi_and_declare_the_font`). Characters outside WinAnsi become `?`.
- **A field with `/DA`** (its own, its ancestors' or the AcroForm's, `form::parse`; [Form fill](form-fill.md) substitutes a widget's own) is drawn in the font the last `Tf` of `/DA` names, looked up in the AcroForm `/DR /Font` of the modifier's document (`dr_font`), and the appearance's `/Resources /Font` declares it under that name with the `/DR` entry as it is (the same reference when `/DR` holds one). The text goes through `font::FontEncoder` ([Font encodings](font-encodings.md)). Maintainer decisions in #178's triage (2026-10-10): combo boxes, list boxes (every option shown) and push-button captions follow the same path as text fields; Type0 fonts are in scope; layout is #325's.
- **The font selection**: text fields and combo boxes write `/DA` through `FieldFont::select_at`, verbatim when it sets the size used and with its last `Tf` size replaced otherwise, so an auto size (`0 Tf`) is written as the size computed (#325). List boxes and push buttons write `/DA` verbatim (`FieldFont::select`), so with `0 Tf` their text is set at size 0 (inferred, not run).
- **A value the `/DR` font cannot hold produces no appearance**: `generate_field_appearance` returns `AppearanceError::NotEncodable` and adds nothing to the modifier, so the viewer draws the field with its own fonts under `/NeedAppearances` (maintainer decision, #178 triage, 2026-10-10: no `?` substitution; a caller-supplied substitute is #175's). Also `NotEncodable`: `/DA` naming no font, a name missing from `/DR`, no `/DR`, and the fonts `FontEncoder::new` refuses. The other errors are `NoRect` (no `/Rect`, or no area) and `Signature`; until #178 all three were `None`.
- Every field that shows text writes `BT`, `0 g`, then the font selection, so the fill colour `/DA` sets is the one the text is drawn in and black is only the default. Until #340 a text field wrote `0 g` after `{da}` and drew `/DA (/Helv 12 Tf 1 0 0 rg)` black (measured on `ae3772a`, #340); `every_field_draws_its_text_in_the_colour_da_sets` in `tests/form_da_font.rs`.
- `Tj` strings are written with `string_syntax` (#29), coordinates as `Number` (#90; `test_non_finite_sizes_are_written_as_numbers`) — [Content text encoding](../invariant/content-text-encoding.md), [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md).
- Values go through `value_as_string` (`from_utf8_lossy`), so a UTF-16BE value becomes U+FFFD — [Text string encoding](../invariant/text-string-encoding.md).

### Background and border (`WidgetLook`, #333)
- Read from the first of `FormField::widgets` in the modifier. Form fill passes a copy whose `widgets` is the one widget it redraws (`redraw_widgets`, #344).
- Rules:
  - `/MK /BG` is filled as `0 0 w h re f`.
  - `/MK /BC` is stroked at border width `b` as `b/2 b/2 w−b h−b re S`.
  - `b` is `/BS /W`, else `/Border[2]`, else 1. A border with `b = 0` is not drawn.
  - A colour array of 1, 3 or 4 components is written as `g`/`rg`/`k` (fill) or `G`/`RG`/`K` (stroke) (`colour_operator`). Any other length, 0 included, is no colour.
  - Without `BG` there is no background, and without `BC` no border.
  - A radio button fills and strokes its circle of radius `min(w, h)/2 − b/2`.
  - The stroke is solid whatever `/BS /S` says.
- These rules come from MuPDF `pdf_write_tx_widget_appearance` (`pdf-appearance.c`) and pdf.js `getBorderAndBackgroundAppearances` (raw source, read 2026-10-23, #333 triage). Both draw nothing without `/MK` and ignore the style. The `b/2` inset and the width fallback are derivations from MuPDF.
- Maintainer decisions (#333 triage, 2026-10-23): no `/MK` means no background or border; only solid borders; every field type follows `/MK`, the push button included (its bevelled look went).
- Until #333 every type drew a fixed look: a text field `0.75 g` with a black 0.5 pt border, other fields `1 g`, a push button a `0.85 g` bevel; `/MK` was ignored (measured, #333).
- What stays per type, drawn after the background and border: the checkmark (`checkbox_appearance`), the radio dot (`radio_appearance`, a circle of half the radius), the combo box's `0.9 g` arrow area `h.min(20)` wide with its triangle (`combo_appearance`), the list box's highlight and text (`list_appearance`), the push button's caption (`button_appearance`).

### Single-line text (`single_line`, `line_text`, #325)
A text field or combo box lays its value out on one line. The constants and formulas follow pdf.js `TextWidgetAnnotation` (`_getAppearance`, `_computeFontSize`, `_renderText`; `LINE_FACTOR` 1.35 and `LINE_DESCENT_FACTOR` 0.35 from `src/shared/util.js`), read from raw source on 2026-10-23.
- **Text box** (GLOSSARY): the widget rectangle inset by the border width `b`, `[b, b, w − 2b, h − 2b]`. A combo box also leaves out its arrow area. The value is clipped to it, as MuPDF clips (`pdf_write_tx_widget_appearance`); pdf.js does not clip.
- **Size** (`line_size`): the `/DA` size when non-zero (10 without `/DA`). With `0 Tf` it is `floor₂(min((bh − 2)/1.35, (bw − 4)/em))`, where `em` is the value's width at size 1. No value is drawn when the box is no wider than 4 or no taller than 2. A fixed size that overflows is clipped, not shrunk.
- **x**: `/Q` 0 puts it at `x0 + 2`, 1 at `x0 + (bw − tw)/2`, 2 at `x0 + bw − tw − 2`.
- **y** (`baseline`): the baseline is at `y0 + (bh − (asc + desc)·size)/2 + desc·size`.
- **`/Q`**: the widget's own `/Q` (0–2, `WidgetLook::quadding`), else `FormField::quadding`: the field's, its ancestors', the AcroForm's, else 0. pdf.js reads it the same way (`getInheritableProperty` on the widget); MuPDF reads only the widget's. Maintainer decision (#325, 2026-10-23): widget first.
- **Ascent and descent**: from the font descriptor unless both are 0, else the Standard 14 AFM values (`font::FontEncoder::ascent_descent`, `standard14_ascent_descent`). Measured from the Adobe AFMs in Apache PDFBox (#325): Helvetica 718/207, Times 683/217, Courier 629/157; Symbol and ZapfDingbats have none. Otherwise pdf.js's fallback, 1.0/0.35 em (`FieldFont::ascent_descent`).
- **Widths**: the `/DR` font's `/Widths` (or the Standard 14 table) by code, a Type0 font's `/W` by CID. Without `/DA`, the WinAnsi Helvetica table.
- **Structure**: `/Tx BMC q`, background and border, the clip `re W n`, `BT 0 g {da} x y Td (…) Tj ET`, then `Q EMC`. The background and border are **inside** the marked content, as in MuPDF and pdf.js. Maintainer decision (#325, 2026-10-23): this replaces #325's triage call to put them outside, which had been made without reading either reference. A password field, or one with an empty value, gives `/Tx BMC q {decoration} Q EMC`.
- Until #325 every value was drawn at `2 (h − 10)/2 Td`, and `0 Tf` drew it invisibly (inferred, #325).

### Multiline and comb (#332)
- **A multiline text field** (`MULTILINE`, `multiline_text`) splits its value at `\r\n`, `\r` and `\n`. Each line is wrapped by `split_line`, which follows pdf.js `_splitLine`:
  - a line within `bw − 2` stays whole
  - otherwise it breaks after the last space seen, else before the character that overflows `bw − 4`
  - after a space break, pdf.js restarts its width count one character past the new chunk's first (`i = lastSpacePos + 1`, then the loop's `i++`). `split_line` does the same, so the chunks match pdf.js's.
- **Line height and size** (pdf.js `_computeFontSize` with a line count):
  - with a fixed size `s`, the line height is `height / max(1, round(height / (1.35 s)))`, where `height = bh − 2`; the `max(1, …)` is justpdf's, where pdf.js divides by 0
  - with `0 Tf`, the line count `n` starts at `max(round(height / 16.2), lines)`, the size is `floor₂(height / n / 1.35)`, and `n` grows (doubling, then binary search) until the wrapped lines fit: chunk count × size ≤ height
- **Placement** (`_getMultilineAppearance`):
  - the first line sits `vPadding + lineHeight − descent` below the text box top, each further chunk `lineHeight` lower
  - `vPadding = min(floor((bh − size)/2), 1)`; `descent = max(lineHeight · 0.35/1.35, font descent · size)`
  - each chunk is aligned by `/Q` with padding 2, or 1 when an unwrappable line overflows
- **A comb** (`comb_text`, `_getCombAppearance`) is drawn when the field has `COMB`, is not multiline, password or file-select, and `FormField::max_len > 0`.
  - Each character is centred in one of `max_len` cells of `bw / max_len`.
  - `/Q` 1 shifts the run by whole cells (`floor((bw − run)/(2 cell)) · cell`), 2 by `bw − run`.
  - A value longer than `max_len` is drawn in full and clipped.
  - Maintainer decision (#332 triage, 2026-10-23): follow pdf.js throughout, an over-long comb value included. Combo boxes stay single-line.

### List box and push button
- **A list box** highlights (`0.6 0.75 1 rg`) every option whose export value is in `FormField::selected_values()` and draws each option's display text (#346, pdf.js `ChoiceWidgetAnnotation._getAppearance`), at a fixed 12-unit line height from the top, stopping when the box runs out. A combo box draws its value as it is.
- **A push button** draws `value_as_string()` (its `/V`) at `4 (h − 10)/2 Td`, not `/MK /CA` (inferred, `button_appearance`).

### Inputs and bounds
- The value and the option list come from the file or the caller; nothing bounds their length. Single-line layout is linear in the value; multiline auto size wraps the value once per size tried, and the tries are logarithmic in the line count (inferred). A list box draws at most `h / 12` options.

## Code
- `justpdf-core/src/form/appearance.rs` — `AppearanceError`, `generate_field_appearance`, `shows_text`, `FieldFont`, `dr_font`, `WidgetLook`, `CIRCLE_K`, `colour_operator`, `text_field_appearance`, `single_line`, `line_size`, `floor2`, `baseline`, `line_text`, `comb_text`, `multiline_text`, `split_line`, `checkbox_appearance`, `radio_appearance`, `combo_appearance`, `list_appearance`, `button_appearance`, `append_circle`
- `justpdf-core/tests/form_da_font.rs`, `justpdf-core/tests/form_layout.rs`, `justpdf-core/tests/form_mk.rs` — `/DA` font, layout and `/MK` tests

## Reference behaviour
- MuPDF `source/pdf/pdf-appearance.c` (`pdf_write_tx_widget_appearance`) and pdf.js `src/core/annotation.js` (`TextWidgetAnnotation`, `ChoiceWidgetAnnotation`, `getBorderAndBackgroundAppearances`), raw source read 2026-10-23 for #325, #332, #333, #346.
- Clauses to compare against: ISO 32000-2 §12.7.4.3 (variable text), §12.5.6.19 (`/MK`).

## Cross-cutting invariants
- [Content text encoding](../invariant/content-text-encoding.md)
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md)
- [Text string encoding](../invariant/text-string-encoding.md) — decoding field values.

## Blast radius
- [Form fill](form-fill.md) — calls this for each widget of a text field, combo box or list box it fills and keeps the old `/AP` on any error; a new error case here becomes a silently undrawn widget there.
- [Form flatten](form-flatten.md) — places the stream by its `/BBox [0 0 w h]` and no `/Matrix`; a change to either moves the flattened field.
- [Font encodings](font-encodings.md) — `FontEncoder` decides which `/DR` fonts are refused (`NotEncodable`) and gives the widths and metrics the layout uses; a change there changes which fields get an appearance and where the text sits.
- [AcroForm](acroform.md) — the inherited `/DA`, `/Q` and `/MaxLen` and the option model drawn here come from the field model.
- [Annotation appearance](annotation-appearance.md), [Signature appearance](signature-appearance.md) — the two other generators; all three draw justpdf-chosen text in a WinAnsi Helvetica, the rule #175 would replace for all of them.
- [Document builder](document-builder.md) — where justpdf picks the font, the appearance declares it with the builder's `standard_font_dict`; a change to that dictionary's encoding changes what every generated appearance's `Tj` bytes mean.

## Known holes / open
- A list box or push button with an auto-size `/DA` (`0 Tf`) is written at size 0 (inferred, not run).
- A push button draws `/V` as its caption, where fill's decision (#344) names `/MK /CA` as the caption; `/MK /CA`, `/R`, `/I` and `/BS /S` styles other than solid are not drawn (not tracked).
- Open question (maintainer): should the push-button caption come from `/MK /CA` instead of `/V`, as #344's triage describes it?
- Text beyond WinAnsi through a caller-supplied appearance font. Tracked: #175
- Text strings read as lossy UTF-8. Tracked: #33
- Standard 14 widths are approximate. Tracked: #292
- The Type0 width path (CID widths from `/W`) used for auto size and alignment has no test. Tracked: #360
- A subset `/FontFile3 /Subtype /OpenType` `/DR` font is refused, so its fields get no appearance. Tracked: #361
