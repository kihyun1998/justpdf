# Object model

## What it is
The PDF value tree (`PdfObject`, `PdfDict`, `IndirectRef`) and the recursive parser that builds direct objects, `N M obj … endobj` wrappers and stream bodies from [Tokenizer](tokenizer.md) tokens. `PdfDict` is a `BTreeMap`, so its keys iterate in byte order, not file order. Almost every crate handles PDF through these types (re-exported from `justpdf-core/src/lib.rs`). The `Display` implementation of the same types is the write-side serializer; that part belongs to [Object serialization](object-serialization.md).

## Governing decisions
**None.** [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) only places the object model in the core layer.

## Design model
Rules read from the code (*inferred* unless marked).
- An integer followed by an integer and `R` is a reference; otherwise `parse_object` seeks back and returns the integer. The two numbers are narrowed with `as`: the object number to `u32`, the generation to `u16`, so `-1 0 R` or `4294967297 0 R` names another object. `parse_indirect_object` narrows the header numbers the same way.
- A stream's `/Length` is used only when it is a direct `Integer` (`read_stream_data`). An indirect `/Length` falls back to scanning for the first `endstream` after the data start (`find_stream_data_by_endstream`), and so does a missing one. A direct `/Length` that runs past the end of the input is an `UnexpectedEof` error, with no scan fallback.
- A direct negative `/Length` panics. `*n as usize` wraps it and the slice `data[pos..pos + length]` ends before it starts. *Measured* 2026-10-30, release `justpdf text` on a page whose content stream has `/Length -1`: exit 101, panic at `object/mod.rs` "slice index starts at 234 but ends at 233".
- A missing `endstream` or `endobj` is tolerated: the parser seeks back and returns what it has.
- `PdfDict` getters (`get_i64`, `get_array`, `get_dict`, …) do not follow references. Resolving is `PdfDocument::resolve` in [Document access](document-access.md). Because of this split, a value the file gives as an indirect object where the reader calls a direct getter is silently absent. One case is a page box given as an indirect array ([Page tree](page-tree.md)).
- **Input bound:** arrays and dictionaries recurse through `parse_object` with no depth limit, so the depth that overflows depends on the thread's stack. *Measured* 2026-10-30, macOS release `justpdf info` (main thread): a catalog value nested 10,000 arrays deep opens; 100,000 deep aborts with a stack overflow (exit 134). Measured in #122 on Windows with a 1 MB main-thread stack (2026-10-07): debug overflows between 400 and 500, release between 1,500 and 2,000; and an object stream holding one object of 200,000 `[` aborted `resolve` (2026-09-30). The element count of an array or dictionary is bounded only by the input.

## Code
- `justpdf-core/src/object/types.rs` — `PdfObject`, `PdfDict`, `IndirectRef`, `PdfDict::get_i64`, `PdfDict::get_ref`, `PdfDict::get_array`, `PdfDict::get_dict`, `PdfDict::get_name`, `PdfDict::get_f64`, `PdfObject::as_stream`
- `justpdf-core/src/object/mod.rs` — `parse_object`, `parse_indirect_object`, `parse_dict_body`, `read_stream_data`, `find_stream_data_by_endstream`

## Reference behaviour
**None.** Not compared. Clauses to compare against: ISO 32000-2 §7.3 (objects), §7.3.8 (streams). These clause numbers were not checked against the spec text.

## Cross-cutting invariants
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md) — this parser is the judge of write-side output.

## Blast radius
- [Tokenizer](tokenizer.md) — the parser depends on reading ahead and seeking back over tokens. A tokenizer change to what a token consumes changes reference and array parsing.
- [Object serialization](object-serialization.md) — a `PdfObject` variant added or changed must change `Display` with it, or written files stop reading back.
- [Document access](document-access.md) — every object load goes through `parse_indirect_object`. A change to what it tolerates (missing `endobj`, a scanned stream length) changes what `resolve` returns for a damaged file.
- [Object streams](object-streams.md) — compressed objects are read with `parse_object` at an offset in the decoded data. Parsing must stop at the object's end, because nothing bounds an object by the next offset.
- [Xref](xref.md), [Repair](repair.md), [Linearization](linearization.md) — parse trailers, xref streams and the first object with these functions, and inherit the same unbounded recursion and `/Length` handling.
- Every consuming crate — a public API change to `PdfObject` or `PdfDict` reaches it. List the consumers with `rg -l 'PdfObject|PdfDict' --glob '*.rs' --glob '!target' .`.

## Known holes / open
- A stream with an indirect `/Length` is cut at the first `endstream` byte sequence, so stream data containing that sequence is truncated (inferred).
- A direct negative `/Length` panics the process (measured above). Tracked: #411.
- Deep array or dictionary nesting overflows the stack (measured above). Decided in #122 (maintainer, 2026-10-07): one core depth limit of 256 for the object parser and every recursive tree walker, past which core returns `LimitExceeded`. Tracked: #122
- Reference and header numbers wrap through `as u32` / `as u16` instead of being refused (inferred). No issue tracks it.
