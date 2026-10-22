# Form fill

## What it is
Sets a value on a field (`/V`, and `/AS` on each widget of a button), sets `/NeedAppearances`. Refuses read-only fields.

## Governing decisions
**None.**

## Design model
- **`set_field_value` writes `/V` on the field and sets the AcroForm's `/NeedAppearances true`** (direct or indirect AcroForm, the rest of it kept), so a viewer redraws the value (#204). It does not regenerate the appearance stream: `generate_field_appearance` still has no caller in product code (maintainer decision, 2026-10-22, #204: a separate change).
- **A checkbox or radio button's `/AS` goes on each widget** (`FormField::widgets`, [AcroForm](acroform.md)): the value where the widget's appearance states (`/AP /N`, else `/AP /D`) include it or the widget has no `/AP`, `/Off` otherwise. Nothing is written on a parent field that is not a widget. Until #204 `/AS` went on the field dictionary, which a viewer ignores when the widgets are kids (measured, `tests/form_fill.rs`).
- **`toggle_checkbox`'s on-state is the first appearance state of the field's widgets that is not `/Off`**, `/Yes` only when they have none; until #204 it was always `/Yes`, which matched no appearance of a box whose on-state is `/On` (measured).

## Code
- `justpdf-core/src/form/fill.rs` — `set_field_value`, `toggle_checkbox`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Form appearance](form-appearance.md) — the side it should be wired to.
- [AcroForm](acroform.md) — the field model.
- [Document modifier](document-modifier.md) — the save path.

## Known holes / open
- A filled value shows only where the viewer honours `/NeedAppearances`; justpdf's own renderer draws the old appearance stream (inferred, not run).
- Tracked: #204 (form fill; appearance regeneration)
