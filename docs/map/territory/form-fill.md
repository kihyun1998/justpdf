# Form fill

## What it is
Sets a field's value (`set_field_value`): `/V` on the field, `/AS` on each widget of a checkbox or radio button, a new normal appearance on each widget of a text field, combo box or list box, `/I` on a choice field that has it, and `/NeedAppearances true` on the AcroForm. Refuses read-only fields. Also reads a value by name (`get_field_value`) and toggles a checkbox (`toggle_checkbox`).

## Governing decisions
**None.** The rules below are maintainer decisions recorded on issues (#204, #344, #346, #347), not in a decision record.

## Design model
- **The field is found by `FormField::name`** in an `AcroForm` the caller parsed ([AcroForm](acroform.md)); not found is `FormError`, as is a read-only field (`FieldFlags::is_read_only`) or a field object that is not a dictionary in the modifier.
- **`/V` and `/NeedAppearances`** (#204): `/V` is written on the field dictionary, and the AcroForm gets `/NeedAppearances true` whether it is direct in the catalog or indirect, the rest of it kept (`set_need_appearances`), so a viewer redraws the value. `/NeedAppearances` stays `true` after every fill, whether or not every widget was redrawn (maintainer decision, #344 triage, 2026-10-22).
- **A text field, combo box or list box gets a new normal appearance on each widget** (#344): `generate_field_appearance` ([Form appearance](form-appearance.md)) on a copy of the field with the new value, that widget's own `/Rect` (`FormField::rect` is only the first widget's) and `widgets` set to that one widget, and the widget's `/AP` becomes the direct `<< /N … >>` (`redraw_widgets`).
  - Maintainer decisions in #344's triage (2026-10-22): only these three types — checkboxes and radio buttons select among their existing appearances through `/AS`, and a push button's caption is `/MK /CA`, not `/V`; the old `/D` and `/R` do not survive, so no state shows the old value.
  - Until #344 every widget kept its old `/AP` and justpdf's renderer drew the old value.
- **A widget `/Rect` written upper-right first is redrawn like any other**: `Rect::from_pdf_array` normalises every `/Rect` it reads (#345), and `Rect::width`/`height` take the absolute value; `/Rect [100 20 0 0]` gets a new `/N` with `/BBox [0 0 100 20]` (measured, #345 triage, 2026-10-23) — [Page tree](page-tree.md) owns `Rect`.
- **Each widget is drawn with its own `/DA`** where its dictionary has one (a string, direct or a reference), else the field's — the field's own, its ancestors', then the AcroForm's, as `form::parse` reads it (#347). It is resolved per widget in `redraw_widgets`, beside the `/Rect`; `FormField` carries only the field's (maintainer decision, #347 triage, 2026-10-23). pdf.js (`Annotation.setDefaultAppearance`, `getInheritableProperty` on the widget) and MuPDF (`pdf_dict_get_inheritable(annot->obj, DA)`) read the widget first (raw source, #347 triage, 2026-10-23). Until #347 every widget was drawn with the field's (measured, #347).
- **A choice field that has `/I` gets it rewritten** to the ascending indices of the options whose export value is in `selected_values()` of the new value (#346). A field without `/I` gets none. This follows pdf.js (`ChoiceWidgetAnnotation`, which writes `/I` on save only where the field had one); pdf.js reports that Acrobat reads `/I` before `/V` (bug 1770750; not checked in Acrobat). Maintainer decision (#346 triage, 2026-10-23). The widgets are redrawn from the same selection.
- **A widget the generator cannot draw keeps its `/AP` as it was**, and the fill still succeeds: any `AppearanceError` — `NotEncodable` (the `/DA` font cannot hold the value; the viewer redraws it under `/NeedAppearances`, #178's triage) and `NoRect` (`redraw_widgets`, `let Ok(..) else continue`).
- **An indirect `/AP` is re-pointed, never rewritten in place**: the widget's `/AP` entry becomes a new direct dictionary and the old object is left as it is, since other widgets may share it (derivation, #344; `an_indirect_appearance_dictionary_is_left_for_the_widgets_sharing_it` in `tests/form_fill.rs`).
- **A checkbox or radio button's `/AS` goes on each widget** (`FormField::widgets`) when the new value is a name: the value where the widget's appearance states (`/AP /N`, else `/AP /D`, `appearance_states`) include it or the widget has no `/AP`, `/Off` otherwise. Nothing is written on a parent field that is not a widget. Until #204 `/AS` went on the field dictionary, which a viewer ignores when the widgets are kids (measured, `tests/form_fill.rs`, #204).
- **`toggle_checkbox`'s on-state is the first appearance state of the field's widgets that is not `/Off`**, `/Yes` only when they have none (`checkbox_on_state`); until #204 it was always `/Yes`, which matched no appearance of a box whose on-state is `/On` (measured, #204).
- Objects are read from the modifier only (`dict_at`, `direct` use `find_object_pub`), not from the source document.
- **Bounds on input**: the value is the caller's `PdfObject`; nothing bounds its size here. The work is one appearance per widget.

## Code
- `justpdf-core/src/form/fill.rs` — `get_field_value`, `set_field_value`, `redraw_widgets`, `dict_at`, `direct`, `appearance_states`, `checkbox_on_state`, `set_need_appearances`, `toggle_checkbox`
- `justpdf-core/tests/form_fill.rs` — fill tests against built documents

## Reference behaviour
- pdf.js `ChoiceWidgetAnnotation` (`/I` on save, #346) and pdf.js / MuPDF widget-first `/DA` (#347), raw source read 2026-10-23 as cited above.

## Cross-cutting invariants
**None.**

## Blast radius
- [Form appearance](form-appearance.md) — fill hands it a field copy whose `widgets`, `rect` and `default_appearance` are one widget's; the generator must read the look and `/Q` from `widgets[0]` and nothing field-wide, or a redraw takes another widget's look.
- [AcroForm](acroform.md) — fill writes where the model says the field and its widgets are (`field_ref`, `widgets`); a change to the field/widget split moves `/V` and `/AS`.
- [Form flatten](form-flatten.md) — draws what fill left in the modifier (#40); a value filled without a new appearance is flattened with the old one.
- [Document modifier](document-modifier.md) — every write is `set_object` in the modifier, and reads see only the modifier's objects.

## Known holes / open
- A value the `/DR` font cannot hold shows only where the viewer honours `/NeedAppearances`; justpdf's own renderer draws the old appearance stream (inferred, not run).
- A push button's value is never drawn by fill, by decision (#344); the generator itself draws a push button's `/V` as its caption (see [Form appearance](form-appearance.md)).
- Text beyond the `/DR` font through a caller-supplied substitute font. Tracked: #175
