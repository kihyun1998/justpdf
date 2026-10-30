# Journal

## What it is
An undo/redo stack for object add, modify, delete and batch operations (`Journal`, `Operation`, `BatchBuilder`), plus its own binary serialization format starting with the `JRNL` magic and version 1 (inferred, `journal.rs`). Its module comment says the caller is responsible for actually applying the inverse operations; the journal only tracks state.

## Governing decisions
**None.** No ADR or issue decides anything about the journal.

## Design model
- **Format.** A length-prefixed little-endian binary format, not PDF syntax: a tag byte per operation and per object kind, `u32` lengths and counts, `i64` integers, `f64` reals and `u16` generations (inferred, `serialize_operation`, `serialize_pdf_object`). Every `PdfObject` value round-trips exactly, including reals and stream data, because nothing passes through the PDF write rules (inferred).
- **Recording.** `record` clears the redo stack and pushes; with recording stopped it drops the operation (inferred). `undo` and `redo` move one operation between the two stacks and return it. `BatchBuilder::commit` records a non-empty batch as one `Operation::Batch`; `cancel` drops it (inferred).
- **Inverses.** `inverse` turns `AddObject` into `DeleteObject` with a `Null` old value, `DeleteObject` into `AddObject` without its value, keeps `ModifyObject` as it is, and reverses a batch (inferred). An operation does not carry the new value, so the journal alone cannot redo a modification or re-add a deleted object.
- **Input size**: `from_bytes` takes outside bytes and checks only the magic, the version and that each read stays within the input (inferred, `Cursor::read_bytes`). It bounds neither the counts nor the nesting depth. `Vec::with_capacity` is called with the stored count before any entry is read. Batches, arrays and dictionaries are read recursively with no depth limit. Measured 2026-10-30 (debug build, a scratch crate against HEAD): a 12-byte input whose undo count is `u32::MAX` returns `None` on macOS; a 5 MB input of one million nested arrays aborts the process with a stack overflow.

## Code
- `justpdf-core/src/journal.rs` — `Operation`, `Journal`, `record`, `undo`, `redo`, `inverse`, `begin_batch`, `to_bytes`, `from_bytes`, `BatchBuilder`, `commit`, `cancel`, `serialize_operation`, `serialize_pdf_object`, `serialize_pdf_dict`, `Cursor`, `deserialize_operation`, `deserialize_pdf_object`

## Reference behaviour
**None.** The module comment cites "Phase 7, §7.10", which names no document in the repo; nothing was compared with another implementation.

## Cross-cutting invariants
**None.**

## Blast radius
- [Object model](object-model.md) — `serialize_pdf_object` and `deserialize_pdf_object` match on every `PdfObject` variant with a fixed tag per variant. A new variant needs a new tag, and changing a tag makes journals written earlier unreadable.

## Known holes / open
- Nothing consumes it beyond the `pub mod journal` declaration in `lib.rs` (inferred, `git grep -n journal -- 'justpdf*/src'`). It is not connected to the [Document modifier](document-modifier.md).
- `from_bytes` on untrusted input can abort the process through unbounded recursion (measured, Design model). Tracked: #425
- Inverses do not carry enough to redo a modification or re-add a deleted object (inferred, `inverse`).
- Is the journal meant to be kept, wired into the modifier, or removed? Only the maintainer can answer.
