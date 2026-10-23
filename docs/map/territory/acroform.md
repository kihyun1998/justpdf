# AcroForm field model

## What it is
Walks the `/AcroForm /Fields` tree and builds a model of the fields (text, checkbox, radio, choice, signature). Passes inherited attributes (`FT`, `Ff`, `DA`) down.

## Governing decisions
**None.**

## Design model
- A field that has its own ancestor as a kid gives `CircularReference`, and going over the visit budget through shared fields gives `LimitExceeded` — [Tree traversal cycles](../invariant/tree-traversal-cycles.md). `/T` is decoded lossily — [Text string encoding](../invariant/text-string-encoding.md).
- `page_obj_num` is always `None`.
- **Choice options and selection** (#346):
  - `FormField::options` holds a `ChoiceOption` per `/Opt` entry. A pair `[export display]` gives both values; a string gives the same text for both.
  - `FormField::indices` is `/I`.
  - `selected_values()` gives the export values of the options at `indices` when it is not empty, else `/V`: a string as one value, an array as its strings.
  - This follows pdf.js `ChoiceWidgetAnnotation` (raw source, read 2026-10-23), where `/I` comes first ("Acrobat itself is using it whatever the V entry is").
  - Until #346, `options` held display texts only, so an export value that differed from its display text matched nothing, and an array `/V` read as nothing (measured, #346 triage).
  - `/Opt` is read from the field only. pdf.js also inherits it from a parent field (its issue 8094); that is not done here.
- **`FormField::max_len`** is the field's `/MaxLen` when it is a non-negative integer (another value counts as 0), else its nearest ancestor's, else 0 (#332).
- **`FormField::quadding`** is the field's `/Q` when it is 0, 1 or 2, else its nearest ancestor's, else the AcroForm's, else 0 (#325).
- **One `FormField` per terminal field** (#204): a kid with neither `/T` nor `/Kids` is a widget of its parent, collected in `FormField::widgets`; the parent is the field, holding `/V`, and its `rect` is its own or its first widget's. A field and widget in one dictionary (no `/Kids`) has itself as its only widget. Kids that are fields are walked as before; a field with both kinds of kid is a field of its own as well. Until #204 each widget kid became a separate field with the parent's name and no `/V`, so a radio group read as two fields valued `None` and `set_field_value` wrote `/V` on the first widget (measured, `tests/form_fill.rs` against master). Maintainer decision (2026-10-22, #204): fix the model rather than have fill follow `/Parent`, so the field tree is walked in one place. Each widget kid is charged to the visit budget as a field was.
- `/Opt` pairs take the display string.

## Code
- `justpdf-core/src/form/types.rs` — `FieldType`, `FieldFlags`, `FormField`, `AcroForm`, `value_as_string`
- `justpdf-core/src/form/parse.rs` — `parse_acroform`, `walk_field_tree`

## Reference behaviour
**None.** Clause to compare against: ISO 32000-2 §12.7.

## Cross-cutting invariants
- [Text string encoding](../invariant/text-string-encoding.md)
- [Tree traversal cycles](../invariant/tree-traversal-cycles.md)

## Blast radius
- [Form fill](form-fill.md), [Form flatten](form-flatten.md), [Form appearance](form-appearance.md) — features built on this model.
- [Signature detection](signature-detection.md) — walks the same tree separately.
- [Facade](facade.md) — `form_fields`.

## Known holes / open
- There is no depth limit — an extreme depth without a cycle can overflow the stack (inferred).
- `walk_field_tree` builds each field's `full_name` by `format!`-ing the parent's name onto the partial name, so every field holds its own copy of the whole path and the copied text grows with depth, outside the visit budget (inferred, code read at `cf6fbcf`).
- Tracked: #33 (text string encoding), #122 (depth limit), #351 (per-depth `full_name` copy)
