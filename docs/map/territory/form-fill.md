# Form fill

## What it is
Sets a value on a field (`/V`; `/AS` on each widget of a button; a new normal appearance on each widget of a text field, combo box or list box), sets `/NeedAppearances`. Refuses read-only fields.

## Governing decisions
**None.**

## Design model
- **`set_field_value` writes `/V` on the field and sets the AcroForm's `/NeedAppearances true`** (direct or indirect AcroForm, the rest of it kept), so a viewer redraws the value (#204). `/NeedAppearances` stays `true` after every fill, whether or not every widget was redrawn (maintainer decision, #344 triage, 2026-10-22).
- **A text field, combo box or list box gets a new normal appearance on each widget** (#344): `generate_field_appearance` on a copy of the field with the new value and that widget's own `/Rect` (`FormField::rect` is only the first widget's), and the widget's `/AP` becomes the direct `<< /N … >>`. Maintainer decisions in #344's triage (2026-10-22): only these three types — checkboxes and radio buttons select among their existing appearances through `/AS`, a push button's caption is `/MK /CA`, not `/V`; the old `/D` and `/R` do not survive, so no state shows the old value. Until #344 every widget kept its old `/AP` and justpdf's renderer drew the old value.
- **A widget `/Rect` written upper-right first is redrawn like any other**: `Rect::width`/`height` take the absolute value, so `/Rect [100 20 0 0]` gets a new `/N` with `/BBox [0 0 100 20]` (measured, #345 triage, 2026-10-23). Since #345 every `/Rect` read from an array is normalised anyway ([Page tree](page-tree.md)).
- **Each widget is drawn with its own `/DA` where its dictionary has one (a string), else the field's** — the field's own, its ancestors', then the AcroForm's, as `form::parse` reads it (#347). pdf.js (`Annotation.setDefaultAppearance`, `getInheritableProperty` on the widget) and MuPDF (`pdf_dict_get_inheritable(annot->obj, DA)`) read the widget first (raw source, #347 triage, 2026-10-23). It is resolved per widget in `redraw_widgets`, beside the `/Rect`; `FormField` carries only the field's (maintainer decision, #347 triage). Until #347 every widget was drawn with the field's (measured).
- **A choice field that has `/I` gets it rewritten** to the ascending indices of the options whose export value is in the new `/V` (#346). A field without `/I` gets none. This follows pdf.js (`ChoiceWidgetAnnotation`, which writes `/I` on save only where the field had one), and pdf.js reports that Acrobat reads `/I` before `/V` (bug 1770750; not checked in Acrobat). Maintainer decision (#346 triage, 2026-10-23). The widgets are redrawn from the same selection.
- **A widget the generator cannot draw keeps its `/AP` as it was** — `NotEncodable` (the `/DA` font cannot hold the value; the viewer redraws it under `/NeedAppearances`, #178's triage) and `NoRect` — and the fill still succeeds.
- **An indirect `/AP` is re-pointed, never rewritten in place**: the widget's `/AP` entry becomes a new direct dictionary and the old object is left as it is, since other widgets may share it (derivation, #344; `an_indirect_appearance_dictionary_is_left_for_the_widgets_sharing_it`).
- **A checkbox or radio button's `/AS` goes on each widget** (`FormField::widgets`, [AcroForm](acroform.md)): the value where the widget's appearance states (`/AP /N`, else `/AP /D`) include it or the widget has no `/AP`, `/Off` otherwise. Nothing is written on a parent field that is not a widget. Until #204 `/AS` went on the field dictionary, which a viewer ignores when the widgets are kids (measured, `tests/form_fill.rs`).
- **`toggle_checkbox`'s on-state is the first appearance state of the field's widgets that is not `/Off`**, `/Yes` only when they have none; until #204 it was always `/Yes`, which matched no appearance of a box whose on-state is `/On` (measured).

## Code
- `justpdf-core/src/form/fill.rs` — `set_field_value`, `redraw_widgets`, `toggle_checkbox`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Form appearance](form-appearance.md) — the generator a fill calls for each widget.
- [AcroForm](acroform.md) — the field model.
- [Document modifier](document-modifier.md) — the save path.

## Known holes / open
- A value the `/DR` font cannot hold shows only where the viewer honours `/NeedAppearances`; justpdf's own renderer draws the old appearance stream (inferred, not run).
- Tracked: #325, #332 (layout of variable text), #333 (`/MK`, `/BS`), #175 (a caller-supplied substitute font), #206 (justpdf's renderer draws no Flate appearance, so a fill does not show there)
