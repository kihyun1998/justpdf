# Object model

## What it is
The PDF value tree (`PdfObject`, the ordered `PdfDict`, `IndirectRef`), and the recursive parser that builds direct objects, `N M obj … endobj` wrappers and stream bodies straight from tokens. Almost every crate handles PDF through these types (re-exported from `lib.rs`). The `Display` implementation of the same types is the write-side serializer; that part is covered by [Object serialization](object-serialization.md).

## Governing decisions
**None.** [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) only states that the "object model" is in the core layer.

## Design model
Rules read out of the code.
- An integer followed by an integer and `R` is a reference; otherwise it rewinds with `seek`.
- A stream's `/Length` is used only when it is a direct `Integer`. If it is an indirect reference, it falls back to scanning for `endstream`.
- Missing `endstream`/`endobj` is tolerated (rewind).
- `PdfDict` getters do not follow references. Resolving references is the job of `resolve` in [Document access](document-access.md). Because of this boundary, values such as a page box given as an indirect array are silently ignored ([Page tree](page-tree.md)).

## Code
- `justpdf-core/src/object/types.rs` — `PdfObject`, `PdfDict`, `IndirectRef`, `get_i64`, `get_ref`, `get_array`, `get_dict`, `get_name`
- `justpdf-core/src/object/mod.rs` — `parse_object`, `parse_indirect_object`, `parse_dict_body`, `read_stream_data`, `find_stream_data_by_endstream`

## Reference behaviour
**None.** Clauses to compare against: ISO 32000-2 §7.3 (objects), §7.3.8 (streams) — pointers from memory, not checked against the spec text.

## Cross-cutting invariants
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md) — this parser is the judge of write-side output.

## Blast radius
- [Tokenizer](tokenizer.md) — the parser depends on rewinding tokens.
- [Object serialization](object-serialization.md) — adding or changing a `PdfObject` variant must change `Display` along with it.
- [Document access](document-access.md) — every object load goes through `parse_indirect_object`.
- [Object streams](object-streams.md) — compressed objects are also read with `parse_object`.
- A public API change to `PdfObject`/`PdfDict` reaches every consuming crate. Get the list of consumers with a command: `rg -l 'PdfObject|PdfDict' --glob '*.rs' --glob '!target' .`.

## Known holes / open
- An indirect `/Length` is always handled by scanning for `endstream`, so a stream whose data contains the byte sequence `endstream` gets cut short (inferred).
