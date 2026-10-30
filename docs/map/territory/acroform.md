# AcroForm field model

## What it is
Walks the `/AcroForm /Fields` tree (`parse_acroform`, `walk_field_tree`) and builds one `FormField` per terminal field (text, checkbox, radio button, push button, combo box, list box, signature), with its widgets and the attributes it inherits (`/FT`, `/Ff`, `/DA`, `/Q`, `/MaxLen`).

## Governing decisions
**None.** [ADR 0005](../../adr/0005-sanitizing-removes-unpainted-text-by-category.md) leaves form field values, field names and `/Opt` out of sanitizing (they are painted through appearances, so they are redaction's) and puts a field's `/TU` and `/TM` in its annotation-text category; it decides [Sanitize](sanitize.md), not this model. [ADR 0004](../../adr/0004-redaction-never-under-erases.md) decides that redaction detaches widgets it removes and removes a field left without widgets (#212, not built).

## Design model
From reading the source, and from the maintainer decisions cited.
- **Entry**: `/AcroForm` direct or indirect. `/Fields` is read only as a direct array, and only its reference entries are walked (inferred: an indirect `/Fields` array gives an AcroForm with no fields). A field without a known `/FT` (own or inherited) is skipped.
- **Cycles and budget**: a field that lists its own ancestor as a kid is `CircularReference`; going over the visit budget through shared fields is `LimitExceeded` — [Tree traversal cycles](../invariant/tree-traversal-cycles.md). Each widget kid is charged to the budget as a field was (`budget.enter` on the widget). Depth is not bounded (below).
- **One `FormField` per terminal field** (#204): a kid with neither `/T` nor `/Kids` is a widget of its parent, collected in `FormField::widgets`; the parent is the field, holding `/V`, and its `rect` is its own or its first widget's. A field and widget in one dictionary (no `/Kids`) has itself as its only widget. Kids that are fields are walked in turn; a field with both kinds of kid is a field of its own as well.
  - Maintainer decision (2026-10-22, #204): fix the model rather than have fill follow `/Parent`, so the field tree is walked in one place.
  - Until #204 each widget kid became a separate field with the parent's name and no `/V`, so a radio group read as two fields valued `None` and `set_field_value` wrote `/V` on the first widget (measured, `tests/form_fill.rs` against master, #204).
- **Text**: `/T` is decoded lossily (`from_utf8_lossy`) and `full_name` is the parent's name and `.` and the partial name — [Text string encoding](../invariant/text-string-encoding.md). `value_as_string` decodes `/V` the same way.
- **Choice options and selection** (#346):
  - `FormField::options` holds a `ChoiceOption` per `/Opt` entry. A pair `[export display]` gives both values; a string gives the same text for both.
  - `FormField::indices` is `/I` (non-negative integers).
  - `selected_values()` gives the export values of the options at `indices` when it is not empty, else `/V`: a string or name as one value, an array as its strings.
  - This follows pdf.js `ChoiceWidgetAnnotation` (raw source, read 2026-10-23, #346), where `/I` comes first ("Acrobat itself is using it whatever the V entry is").
  - Until #346, `options` held display texts only, so an export value that differed from its display text matched nothing, and an array `/V` read as nothing (measured, #346 triage).
  - `/Opt` is read from the field only. pdf.js also inherits it from a parent field (its issue 8094); that is not done here (#359).
- **`FormField::max_len`** is the field's `/MaxLen` when it is a non-negative integer (another value counts as 0), else its nearest ancestor's, else 0 (#332).
- **`FormField::quadding`** is the field's `/Q` when it is 0, 1 or 2, else its nearest ancestor's, else the AcroForm's, else 0 (#325).
- **`FormField::default_appearance`** is the field's `/DA` string, else its nearest ancestor's, else the AcroForm's. A widget's own `/DA` is not read here; [Form fill](form-fill.md) reads it per widget (#347).
- `page_obj_num` is always `None` (`walk_field_tree`).
- **Bounds on input**: the number of fields and widgets is bounded by the visit budget; the depth is not, and `full_name` is copied per field with the whole path, outside the budget (below).

## Code
- `justpdf-core/src/form/types.rs` — `FieldType`, `FieldFlags`, `ChoiceOption`, `FormField`, `FormField::value_as_string`, `FormField::selected_values`, `FormField::is_checked`, `AcroForm`
- `justpdf-core/src/form/parse.rs` — `parse_acroform`, `InheritedField`, `quadding`, `walk_field_tree`

## Reference behaviour
- pdf.js `ChoiceWidgetAnnotation` for choice selection (#346, raw source read 2026-10-23).
- Clause to compare against: ISO 32000-2 §12.7.

## Cross-cutting invariants
- [Text string encoding](../invariant/text-string-encoding.md)
- [Tree traversal cycles](../invariant/tree-traversal-cycles.md)

## Blast radius
- [Form fill](form-fill.md) — writes `/V` on `field_ref` and `/AS` and `/AP` on each of `widgets`; a change to what counts as a field or a widget changes where fill writes.
- [Form appearance](form-appearance.md) — draws from `FormField` (value, options, indices, flags, `default_appearance`, `quadding`, `max_len`, first widget); an inheritance rule changed here changes every generated appearance.
- [Form flatten](form-flatten.md) — gates on `parse_acroform` returning a form, then walks the pages' `/Annots` itself; a document whose AcroForm this parser rejects is not flattened.
- [Signature detection](signature-detection.md) — walks the same field tree with its own code (`sign/detect.rs — collect_sig_fields`), which neither has #204's widget-kid rule nor inherits `/FT`: a field with `/Kids` is always descended into and only a dictionary with its own `/FT /Sig` counts (inferred). A tree rule changed here is not seen there.
- [Facade](facade.md) — `Document::form_fields` returns `AcroForm` and `FormField`, so a field change is a public API change.

## Known holes / open
- There is no depth limit: an extreme depth without a cycle can overflow the stack (inferred). Tracked: #122
- `walk_field_tree` builds each field's `full_name` by `format!`-ing the parent's name onto the partial name, so every field holds its own copy of the whole path and the copied text grows with depth, outside the visit budget (inferred, code read at `cf6fbcf` and again at 88b2a08). Tracked: #351, #135
- `/Opt` is not inherited from a parent field. Tracked: #359
- An indirect `/Fields` array reads as no fields (inferred, not run).
- Text strings are read as lossy UTF-8, so a UTF-16BE `/T` or `/V` becomes U+FFFD. Tracked: #33
