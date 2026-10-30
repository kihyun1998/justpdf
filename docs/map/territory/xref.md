# Xref and the incremental update chain

## What it is
Finds `startxref`, follows the `/Prev` chain, and merges every section, classic xref table or xref stream, into one `Xref { entries, trailer }`. It is the only source of the mapping from object number to file offset, or to an index inside an object stream.

## Governing decisions
**None.** The trailer rule below was decided on an issue (#26), not in a record.

## Design model
Rules read from the code (*inferred* unless marked).
- **The newest section wins.** Sections are read newest first, and an entry from an earlier section does not overwrite a number already present (`merged.entries.entry(obj_num).or_insert`). This includes a free entry in the newer section.
- **Only the newest trailer is kept** (`if merged.trailer.is_empty()`). Keys from earlier trailers are not merged. This matches pdf.js and MuPDF, and not merging was the maintainer's decision on #26. So an incremental write must carry the keys over itself — [Incremental trailer](../invariant/incremental-trailer.md).
- A `/Prev` cycle is cut by a set of visited offsets. `startxref` is searched for only in the last 1024 bytes (`find_startxref`).
- A section offset at or past EOF is an `InvalidXref` error. `PdfDocument::open` does not fall back to [Repair](repair.md).
- A classic table is recognised only when the bytes at the offset start with `xref`. Anything else is parsed as an xref stream object, so an offset that lands on whitespace before `xref` fails. Table entries are read leniently: any digit runs and whitespace, then `n` or `f`; another type byte is an error (`parse_xref_table`). Because of this leniency the reader accepts 21-byte entries as well as the 20 bytes the spec requires, so a justpdf round trip cannot catch a writer that breaks [Xref entry format](../invariant/xref-entry-format.md).
- **Xref streams** (`parse_xref_stream`): the reader honours `/Index`, else `(0, /Size)`; `/W[0] == 0` means type 1; an unknown type is skipped; a field wider than 8 bytes keeps only its low 64 bits (`read_field`). The writer's layout (`write_xref_stream_with_generations` in [File serialization](file-serialization.md)) is `/W [1 w2 w3]` with no `/Index`, which this reader reads.
- Fields are narrowed with `as`: a type-2 index to `u16`, `gen_num` to `u16`, `obj_stream_num` and `next_free` to `u32`. *Measured* 2026-10-07 (#158): index 65,536 resolves to index 0's object. Decided in #158: widen the index to `u32`, and skip rather than wrap any field that does not fit.
- **Input bound:** the entry count of a table subsection or an xref-stream `/Index` pair is bounded by the bytes present: a table entry at EOF fails on its type byte, and a stream loop stops at the end of the decoded data. The decoded data has no bound of its own ([Stream filters](stream-filters.md), #161). The number of sections is bounded by the visited-offset set. `start + i` is unchecked `u32` arithmetic, so a start number near `u32::MAX` overflows (inferred).

## Code
- `justpdf-core/src/xref/mod.rs` — `find_startxref`, `load_xref`, `load_xref_at`, `parse_xref_stream`, `read_field`
- `justpdf-core/src/xref/table.rs` — `Xref`, `Xref::get`, `XrefEntry`, `XrefEntry::defined_generation`, `parse_xref_table`, `read_ascii_number`

## Reference behaviour
- Trailer merging: pdf.js and MuPDF keep only the newest trailer (#26, as the maintainer was shown it).
- Type-2 index width: pdf.js keeps the index unbounded and rewrites it from the object stream's header numbers (#158, 2026-10-07).
- Not compared otherwise. Clauses to compare against: ISO 32000-2 §7.5.4 (xref table), §7.5.5 (trailer), §7.5.6 (incremental updates), §7.5.8 (xref streams, including hybrid `/XRefStm`). These clause numbers were not checked against the spec text.

## Cross-cutting invariants
- [Incremental trailer](../invariant/incremental-trailer.md) — the read side is here: only the newest trailer survives.

## Blast radius
- [Document access](document-access.md) — every object lookup goes through `Xref::get`, and `defines` trusts the entry's generation. A wrong entry makes `resolve` return another object or `Null`.
- [Object streams](object-streams.md) — a type-2 entry's `index_within` width (`u16`) is also the cap on the ObjStm offset table (`OBJ_STREAM_ADDRESSABLE_INDICES`). Widening one without the other breaks the bound or makes indices unreachable.
- [File serialization](file-serialization.md) — writes the tables and streams this code reads. A layout change on either side must stay readable by the other.
- [Incremental save](incremental-save.md), [Signing](signing.md) — call `find_startxref` (and `load_xref` for the previous trailer in `incremental_save`) to chain a new section by `/Prev`. A change to which trailer survives changes which keys they must carry.
- [Repair](repair.md) — builds the same `Xref` shape from a scan. A field added to `Xref` or `XrefEntry` must be filled there too.

## Known holes / open
- Hybrid-reference files are not read: nothing in the reader looks at `/XRefStm`. The only hits of `rg XRefStm justpdf-core/src` are in `writer/modify.rs`, where `incremental_trailer` drops the key. Tracked: #53
- Xref-stream fields wrap through `as` (above). Tracked: #158
