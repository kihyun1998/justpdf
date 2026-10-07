# Embedded files

## What it is
Reads (including an MD5 check) and adds attachments in the `/Names /EmbeddedFiles` name tree. ZUGFeRD invoice XML is read through this path.

## Governing decisions
**None.**

## Design model
- One of the few places in the repo that decodes PDF text strings correctly (`obj_to_string`, handles the UTF-16BE BOM). No other reading code reuses it.
- Adding appends to the name tree root's `/Names` without sorting (even if the root has `/Kids` — inferred).
- The MIME type is kept as `Name(b"application/pdf")`, and escaping `/` as `#2F` is left to the serializer (#29). When reading, a `#2F` left in the decoded name is turned into `/` — before #29 justpdf wrote `/application#232Fpdf` (escaped twice), and such a file decodes to `application#2Fpdf` (`test_mime_type_written_escaped_twice_still_reads`) — [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md).
- A kid in the name tree that points at an ancestor, and a kid past the visit budget, are skipped — [Tree traversal cycles](../invariant/tree-traversal-cycles.md).
- `/F` and `/UF` are written as UTF-8 as is — [Text string encoding](../invariant/text-string-encoding.md).

## Code
- `justpdf-core/src/embedded_file.rs` — `read_embedded_files`, `extract_file`, `add_embedded_file`, `wire_into_name_tree`, `obj_to_string`

## Reference behaviour
**None.** Clauses to compare against: ISO 32000-2 §7.11.4, §7.9.6 (name trees).

## Cross-cutting invariants
- [Text string encoding](../invariant/text-string-encoding.md)
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md)
- [Tree traversal cycles](../invariant/tree-traversal-cycles.md)

## Blast radius
- [ZUGFeRD](zugferd.md) — the only outside consumer of the reading path.
- [Compress stripping](compress-stripping.md) — deletes the whole name tree in extreme.
- [Stream filters](stream-filters.md) — decoding the file stream.
- [Facade](facade.md) — `embedded_files`.

## Known holes / open
- There is no extraction round-trip test (only reading the MIME back after adding: `test_mime_type_is_written_once_escaped`).
- There is no depth limit — extreme depth without a cycle can overflow the stack (inferred).
- Tracked: #33 (text string encoding), #58 (extreme removes attachments, and high removes the PDF/A + Factur-X XMP; decided: detect and preserve), #122 (depth limit)
