# Linearization

## What it is
The web-optimized (Fast Web View) file format. The read side detects whether the first object is a `/Linearized` dictionary and reads the page offset hint table bit by bit. The write side writes the linearization dictionary with fixed-width numbers and builds the file in passes, in the order hint stream, first-page objects, the rest, main xref.

## Governing decisions
**None.**

## Design model
- Write: numbers are written zero-padded to 10 digits so that "the dictionary size is the same across passes and does not oscillate". The first-page xref is omitted (the comment says the spec allows it, but that is doubtful — inferred).
- Write: the hint stream dictionary is `/Type /XRef` and has no `/S`.
- Read: detection only (no check of `/L` against the actual length). Header items 6–9 are read and discarded. Shared object hints are not parsed.
- The read side reads the hint header as 36 bytes, but per the spec table there are more 16-bit items, so it may be 44 bytes (inferred — needs checking against the spec text).
- The document open path does not use linearization information.

## Code
- `justpdf-core/src/linearized.rs` — `detect_linearization`, `is_linearized`, `read_linearization`, `parse_hint_tables`, `PageOffsetHint`, `BitReader`
- `justpdf-core/src/writer/linearize.rs` — `linearize`, `write_linearized_pdf`, `write_linearized_inner`, `build_hint_stream`, `compute_page_offsets`, `create_pdf_at_generations`, `linearize_keeps_source_generations`

## Reference behaviour
**None.** The code cites "PDF spec section 7.4", "F.3" and "Table F.1", but there is no record of a comparison. Compare against: ISO 32000-2 Annex F.

## Cross-cutting invariants
- [Xref entry format](../invariant/xref-entry-format.md) — the main xref table.
- [Source generation](../invariant/source-generation.md) — source objects are written to the header and main xref entry at the xref generation from `object_refs()`. The newly built linearization dictionary and hint stream are generation 0. The trailer `/Root` and `/Info` are copied from the source as they are — a reference that did not resolve in the source is not brought back.

## Blast radius
- [File serialization](file-serialization.md) — the write side writes the xref and trailer directly.
- [Facade](facade.md) — `is_linearized` is the only consumer that calls the read side.
- [Page tree](page-tree.md) — computing the first-page object set relies on page order.

## Known holes / open
- The write side (`linearize_pdf`) is only re-exported and not called from product code. The CLI has no linearize command either.
- The read side's hint header length and cited table numbers may differ from the spec (verdict withheld until checked against the spec text).
- Tracked: #56 (hint table layout)
- The first-page section holds every page: the first section collects the catalog's dependencies, and catalog → `/Pages` → `/Kids` reaches every page. Only objects referenced from nowhere are left for the remaining section (measured 2026-09-29: in `linearize_keeps_source_generations`, a mutation that changed the second page object's header to generation 0 was not caught by the remaining-section loop, and was caught only once an unreferenced object was added). It is not just the catalog: the reference collector follows every reference, so the dependencies of the first page alone also take in every sibling page through `/Parent` → `/Pages` → `/Kids` (measured 2026-09-30: in a 3-page document with page objects 4, 6, 8, the first page's dependencies alone are `[1, 3, 4, 5, 6, 7, 8]`, with pages 6 and 8 before `/E`). So narrowing only the catalog side does not fix it. Because the per-page split skips first-section objects, pages 2..N in the hint table get an object count of 0 (inferred — code reading). Tracked: #124
