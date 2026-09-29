# Journal

## What it is
An undo/redo stack for object add, modify, delete and batch operations, plus its own binary serialization format starting with the `JRNL` magic. "Actually applying the inverse operation is the caller's responsibility."

## Governing decisions
**None.**

## Design model
- A length-prefixed binary format rather than PDF syntax, so it is safe for object round trips.

## Code
- `justpdf-core/src/journal.rs` — `Journal`, `Operation`, `BatchBuilder`, `inverse`, `to_bytes`, `from_bytes`, `serialize_pdf_object`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Object model](object-model.md) — when `PdfObject` gains a variant, `serialize_pdf_object` has to grow with it.

## Known holes / open
- Nothing consumes it beyond the `pub mod journal` declaration in `lib.rs`. It is not connected to the [Document modifier](document-modifier.md).
