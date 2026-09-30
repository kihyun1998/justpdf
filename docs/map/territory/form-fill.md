# Form fill

## What it is
Sets a value on a field (`/V`; `/AS` for buttons). Refuses read-only fields.

## Governing decisions
**None.**

## Design model
- **It does not regenerate the appearance and does not set `NeedAppearances`**: after the value changes, a viewer still shows the old appearance (inferred). The appearance generator `generate_field_appearance` has no caller in product code.
- The checkbox on-state name is hard-coded as `/Yes` (real files can use a different name).

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
- A filled value does not show on screen (above, inferred; no test checks it by rendering).
- Tracked: #40 (form fill and flatten)
