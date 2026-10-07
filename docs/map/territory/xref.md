# Xref and the incremental update chain

## What it is
Finds `startxref`, follows the `/Prev` chain, and merges every section (classic xref table or xref stream) into one `Xref { entries, trailer }`. The only source of object number → file offset (or position inside an object stream).

## Governing decisions
**None.**

## Design model
Rules read from the code.
- **The newest section wins**: an entry from an earlier section does not overwrite a number already present (`or_insert`).
- **Only the newest trailer is kept.** Keys from earlier sections' trailers are not merged — the same as pdf.js and MuPDF, and not merging is the maintainer decision of #26. So incremental writes carry the keys over — [Incremental trailer](../invariant/incremental-trailer.md).
- A `/Prev` cycle is cut with `visited`. `startxref` is searched for only in the last 1024 bytes of the file.
- Xref streams: `/W[0] == 0` means type 1, and unknown types are skipped. This code reads the layout the writing side's `write_xref_stream` produces (`/W [1 w2 w3]`, no `/Index`).
- An offset past EOF is a hard error and does not fall back to [repair](repair.md) automatically.

## Code
- `justpdf-core/src/xref/mod.rs` — `find_startxref`, `load_xref`, `load_xref_at`, `parse_xref_stream`, `read_field`
- `justpdf-core/src/xref/table.rs` — `Xref`, `XrefEntry`, `parse_xref_table`, `read_ascii_number`

## Reference behaviour
**None.** Clauses to compare against: ISO 32000-2 §7.5.4 (xref table), §7.5.5 (trailer), §7.5.6 (incremental updates), §7.5.8 (xref streams, including hybrid `/XRefStm`) — pointers from memory.

## Cross-cutting invariants
- [Incremental trailer](../invariant/incremental-trailer.md) — the reading-side half is here.

## Blast radius
- [Document access](document-access.md) — every object lookup goes through `Xref::get`.
- [object streams](object-streams.md) — type 2 entries lead to loading a compressed object.
- [File serialization](file-serialization.md) — the writing side of xref tables/streams. When changing the layout, look at both sides.
- [Incremental save](incremental-save.md), [Signing](signing.md) — both use `find_startxref` to write `/Prev` and build a new trailer.
- [repair](repair.md) — the fallback path when the xref is broken (currently only by an explicit call).

## Known holes / open
- Hybrid files (`/XRefStm`) are not handled: `rg XRefStm justpdf-core/src` finds nothing.
- Xref-stream fields are narrowed with `as`: a type-2 index to `u16` (index 65,536 resolves to index 0's object, measured 2026-10-07), `gen_num` to `u16`, `obj_stream_num` and `next_free` to `u32`. Decided in #158: widen the index to `u32` and skip, not wrap, any field that does not fit. pdf.js keeps the index unbounded and rewrites it from the object stream's header numbers; that check here is #109.
- Tracked: #53 (/XRefStm), #158 (field narrowing)
