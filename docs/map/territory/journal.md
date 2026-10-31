# Journal

## What it is
An undo/redo stack for object add, modify, delete and batch operations (`Journal`, `Operation`, `BatchBuilder`), plus its own binary serialization format starting with the `JRNL` magic and version 1 (inferred, `journal.rs`). Its module comment says the caller is responsible for actually applying the inverse operations; the journal only tracks state.

## Governing decisions
**None** in a record. Decided on #425 (maintainer, 2026-10-31): keep the journal and make `from_bytes` safe on untrusted bytes; whether to keep it at all stays open.

## Design model
- **Format.** A length-prefixed little-endian binary format, not PDF syntax: a tag byte per operation and per object kind, `u32` lengths and counts, `i64` integers, `f64` reals and `u16` generations (inferred, `serialize_operation`, `serialize_pdf_object`). Every `PdfObject` value nested at most 256 deep round-trips exactly, including reals and stream data, because nothing passes through the PDF write rules (inferred). `to_bytes` writes deeper values and batches, but `from_bytes` refuses them (below), so such a journal does not read back (inferred; values that deep come from `parse_object`, which has no depth limit yet — #122 — or from nesting batches).
- **Recording.** `record` clears the redo stack and pushes; with recording stopped it drops the operation (inferred). `undo` and `redo` move one operation between the two stacks and return it. `BatchBuilder::commit` records a non-empty batch as one `Operation::Batch`; `cancel` drops it (inferred).
- **Inverses.** `inverse` turns `AddObject` into `DeleteObject` with a `Null` old value, `DeleteObject` into `AddObject` without its value, keeps `ModifyObject` as it is, and reverses a batch (inferred). An operation does not carry the new value, so the journal alone cannot redo a modification or re-add a deleted object.
- **Input size** (#425): `from_bytes` checks the magic, the version and that each read stays within the input (`Cursor::read_bytes`, with checked arithmetic; inferred). Batches, arrays, dictionaries and streams nest at most 256 deep (`MAX_DEPTH`, the colour-space limit of #414), counted together: a batch inside 200 batches leaves 56 levels for its values. Deeper input returns `None` (*measured* by the unit tests). No vector is reserved from a count read from the input; each grows as entries are read, so memory follows the bytes read. A reservation bounded per level by the remaining bytes still stacks across nested levels: 256 arrays each stating `u32::MAX` items over 4,000 nulls kept 57 MB live for a 5.3 KB input, and the unbounded reservation asked 240 GB for a 12-byte input; without reservation every case stays under 2 MB (*measured* 2026-10-31 by `tests/journal_alloc.rs`, a counting allocator). Before #425 a 500 KB input of 100,000 nested arrays aborted the process with a stack overflow (*measured* 2026-10-31, release), and a 12-byte input whose undo count is `u32::MAX` reserved for that count before reading (returned `None` on macOS in 0.6 ms; inferred to abort where the reservation fails). Pinned by the `journal.rs` unit tests `deep_nesting_is_refused_instead_of_overflowing_the_stack`, `nesting_reads_to_256_levels` and `batches_and_objects_share_one_depth`, and by `a_stated_count_reserves_nothing_beyond_what_is_read` in `tests/journal_alloc.rs`; mutations removing either depth check, moving the limit by one, not counting a batch or dictionary level, starting values at depth 0 inside a batch, reserving the raw count, or reserving per level by the remaining bytes each failed (four by aborting the test process; *measured* 2026-10-31).

## Code
- `justpdf-core/src/journal.rs` — `Operation`, `Journal`, `record`, `undo`, `redo`, `inverse`, `begin_batch`, `to_bytes`, `from_bytes`, `BatchBuilder`, `commit`, `cancel`, `serialize_operation`, `serialize_pdf_object`, `serialize_pdf_dict`, `MAX_DEPTH`, `Cursor`, `deserialize_operation`, `deserialize_pdf_object`, `deserialize_pdf_dict`
- `justpdf-core/tests/journal_alloc.rs` — allocation while reading stated counts (a counting global allocator, so one test)

## Reference behaviour
**None.** The module comment cites "Phase 7, §7.10", which names no document in the repo; nothing was compared with another implementation.

## Cross-cutting invariants
**None.**

## Blast radius
- [Object model](object-model.md) — `serialize_pdf_object` and `deserialize_pdf_object` match on every `PdfObject` variant with a fixed tag per variant. A new variant needs a new tag, and changing a tag makes journals written earlier unreadable.

## Known holes / open
- Nothing consumes it beyond the `pub mod journal` declaration in `lib.rs` (inferred, `git grep -n journal -- 'justpdf*/src'`). It is not connected to the [Document modifier](document-modifier.md).
- Inverses do not carry enough to redo a modification or re-add a deleted object (inferred, `inverse`).
- Is the journal meant to be kept, wired into the modifier, or removed? Only the maintainer can answer.
