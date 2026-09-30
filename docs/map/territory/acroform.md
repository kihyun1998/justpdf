# AcroForm field model

## What it is
Walks the `/AcroForm /Fields` tree and builds a model of the fields (text, checkbox, radio, choice, signature). Passes inherited attributes (`FT`, `Ff`, `DA`) down.

## Governing decisions
**None.**

## Design model
- A field that has its own ancestor as a kid gives `CircularReference`, and going over the visit budget through shared fields gives `LimitExceeded` — [Tree traversal cycles](../invariant/tree-traversal-cycles.md). `/T` is decoded lossily — [Text string encoding](../invariant/text-string-encoding.md).
- `page_obj_num` is always `None`.
- A widget child without `/T` becomes a separate "field" carrying the parent's name, with no `/V` (inferred).
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
- Tracked: #33 (text string encoding), #122 (depth limit)
