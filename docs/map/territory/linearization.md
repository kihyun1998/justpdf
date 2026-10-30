# Linearization

## What it is
The web-optimised (Fast Web View) file layout. The read side checks whether the first object after the header is a `/Linearized` dictionary and returns its parameters, and can read a page offset hint table bit by bit. The write side builds the file in passes: the linearization dictionary with fixed-width numbers, the hint stream, the first-page section, the remaining objects, then the main xref.

## Governing decisions
**None.** The layout fix was decided on an issue (#56), not in a record.

## Design model
Rules read from the code (*inferred* unless marked).
- **Read, detection** (`detect_linearization`): skips the header and comment lines, parses one indirect object, and requires `/Linearized`, `/L`, `/O`, `/E`, `/N`, `/T` and a two-element `/H`; any missing key gives `None`. `/L` is not compared with the file length. The document open path does not use the parameters; `read_linearization` re-parses the raw bytes.
- **Read, hint table** (`parse_hint_tables`): reads a 36-byte header as nine `u32`s, uses items 1–5, reads items 6–9 and discards them, then reads per-page object-count and page-length deltas. Shared-object hints are not read. The ISO header is 13 items mixing 32- and 16-bit fields, also 36 bytes: 5 × 4 + 8 × 2 (qpdf `readHPageOffset`, read raw for #56, 2026-10-08). So the size check passes and every field after item 2 is read from the wrong offset. The code's comment cites "Table F.1"; which ISO table that names was not checked here.
- **Write:** numbers in the linearization dictionary are zero-padded to 10 digits, so the dictionary has the same size in every pass. No first-page xref is written; the comment says the spec allows that, which is unchecked (#56 leaves it out of scope until a spec check). The hint stream dictionary is `/Type /XRef` with no `/S`. `build_hint_stream` writes the same nine-`u32` header and only the first two per-page sections (#56, confirmed from code 2026-10-08).
- **Write, first-page section:** it holds the dependencies of the first page, the catalog and `/Info`, collected by `collect_object_deps`, which follows every reference. Catalog → `/Pages` → `/Kids` reaches every page, and so does the first page alone through `/Parent` → `/Pages` → `/Kids`. *Measured* 2026-09-30: in a 3-page document with page objects 4, 6 and 8, the first page's dependencies alone are `[1, 3, 4, 5, 6, 7, 8]`. So the first section holds every page, and only objects referenced from nowhere are left for the rest. *Measured* 2026-09-29: in `linearize_keeps_source_generations`, a mutation changing the second page object's header to generation 0 was not caught by the remaining-section loop, and was caught only once an unreferenced object was added. Pages 2..N get an object count of 0 in the hint table, because the per-page split skips first-section objects (inferred).
- **Write, generations:** source objects are copied from `object_refs()` and written at their xref generation in the header and main xref entry; the new linearization dictionary and hint stream are generation 0; trailer `/Root` and `/Info` are copied as they are (#106, `linearize_keeps_source_generations`). An object whose `resolve` fails is silently left out.
- **Input bound:** `parse_hint_tables` allocates `Vec::with_capacity(n_pages)` from the file's `/N` before reading any data, so a huge or negative `/N` sizes an allocation. It is reached only from tests today. The writer holds every object of the document in memory at once (`all_objects`, `obj_map`).

## Code
- `justpdf-core/src/linearized.rs` — `LinearizationParams`, `detect_linearization`, `is_linearized`, `read_linearization`, `parse_hint_tables`, `PageOffsetHint`, `BitReader`, `skip_header`, `skip_comments`
- `justpdf-core/src/writer/linearize.rs` — `linearize`, `compute_page_object_info`, `collect_object_deps`, `extract_refs`, `build_hint_stream`, `write_linearized_pdf`, `write_linearized_inner`, `compute_page_offsets`, `create_pdf_at_generations`, `linearize_keeps_source_generations`
- `justpdf-core/src/writer/mod.rs` — `linearize_pdf` (re-export of `linearize`)

## Reference behaviour
- Page offset hint table header: qpdf `libqpdf/QPDF_linearization.cc`, `readHPageOffset` (read raw 2026-10-08, #56): 13 items, 32-bit items 1, 2, 4, 6, 8 and 16-bit items 3, 5, 7, 9–13; seven per-page sections; qpdf requires `/S` and fails with `damagedPDF` when it is out of bounds. justpdf's output was not run against qpdf (#56: not installed then).
- The code cites "PDF spec section 7.4", "F.3" and "Table F.1"; ISO 32000-2 Annex F was not compared.

## Cross-cutting invariants
- [Xref entry format](../invariant/xref-entry-format.md) — the main xref in `write_linearized_inner`.
- [Source generation](../invariant/source-generation.md) — source objects keep their xref generation in the header and main xref entry.

## Blast radius
- [File serialization](file-serialization.md) — the write side writes its own xref table and trailer instead of calling the serializer; a change to the entry format or trailer keys there must be repeated here.
- [Page tree](page-tree.md) — the first-page set and the hint table follow `collect_pages` order and the `/Parent` / `/Kids` links; a change to what counts as a page changes both.
- [Facade](facade.md) — `Document::is_linearized` is the only product caller of the read side; it reports presence of the dictionary, not a valid layout.
- [Document access](document-access.md) — `object_refs()` supplies the objects and generations the writer copies; an object `resolve` refuses is dropped from the output.

## Known holes / open
- The write side (`linearize_pdf`) is public but not called from product code, and the CLI has no linearize command (`rg -i lineari justpdf-cli` finds nothing, 2026-10-30).
- Hint table header layout and missing sections, on both sides. Decided in #56: fix both to the 13-item layout with all sections, a minimal shared object table and `/S`, verified against an independent producer or reader. Tracked: #56
- The first-page section holds every page (above). Tracked: #124
- Whether the first-page xref may be omitted is an open spec question (#56 out of scope).
- `parse_hint_tables` sizes allocations from the file's `/N` (above). No issue tracks it.
