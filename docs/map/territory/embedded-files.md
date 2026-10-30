# Embedded files

## What it is
Reads the attachments in the `/Names /EmbeddedFiles` name tree (`read_embedded_files`), extracts an attachment's bytes with an MD5 check (`extract_file`), and adds attachments (`add_embedded_file`, `wire_into_name_tree`). ZUGFeRD invoice XML is read through this path.

## Governing decisions
**None.** [ADR 0005](../../adr/0005-sanitizing-removes-unpainted-text-by-category.md)'s attachments category removes the `EmbeddedFiles` name tree, `/AF` arrays and FileAttachment annotations; it decides [Sanitize](sanitize.md), not this module.

## Design model
From reading the source; no record governs it.
- **Text decoding** (`obj_to_string`, used for `/UF`, `/F`, `/Desc`, `/CreationDate`, `/ModDate`): a string with the UTF-16BE BOM is decoded as UTF-16 (`None` when it is not valid UTF-16); any other string as lossy UTF-8, not PDFDocEncoding. The facade's `metadata_string` (`justpdf/src/lib.rs`) and `font::encoding::decode_text` decode the BOM with their own code; none of the three calls another (grep at 88b2a08) — [Text string encoding](../invariant/text-string-encoding.md).
- **A `FileSpec`** takes `/UF`, else `/F`; `/EF /F` only when it is a reference to a stream; from that stream `/Subtype` (MIME), and `/Params` `/Size`, `/CheckSum`, `/CreationDate`, `/ModDate`.
- **MIME type**: written as `Name(b"application/pdf")`, escaping `/` as `#2F` left to the serializer (#29). When reading, a `#2F` left in the decoded name is turned into `/`: before #29 justpdf wrote `/application#232Fpdf` (escaped twice), and such a file decodes to `application#2Fpdf` (`test_mime_type_written_escaped_twice_still_reads`) — [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md).
- **Name tree** (`collect_name_tree_values`): a kid that points at an ancestor, and a kid past the visit budget, are skipped; an indirect root is put among the ancestors first — [Tree traversal cycles](../invariant/tree-traversal-cycles.md).
- **`extract_file`** decodes the stream through its filters (`stream::decode_stream`) and, when `/CheckSum` is present, fails with `StreamDecode` on an MD5 mismatch.
- **Adding** compresses the data with Flate, writes `/Size` and `/CheckSum`, a `/Filespec` with `/F` and `/UF` as the Rust string's UTF-8 bytes, and appends the name and reference to the name tree **root's** `/Names` without sorting, even when the root has `/Kids` (inferred, `wire_into_name_tree`). A missing `/Names` or `/EmbeddedFiles` is created as an indirect object.
- **Bounds on input**: kids are counted by the visit budget; depth is not bounded (below). A name-tree value that is a reference is resolved and cloned per entry outside the budget (measured on #135: 840 ms at K = M = 4,000). `extract_file` decodes the whole stream into memory with no size bound (Tracked: #161).

## Code
- `justpdf-core/src/embedded_file.rs` — `FileSpec`, `obj_to_string`, `parse_file_spec_dict`, `resolve_dict`, `read_embedded_files`, `collect_name_tree_values`, `extract_file`, `add_embedded_file`, `wire_into_name_tree`

## Reference behaviour
**None.** Clauses to compare against: ISO 32000-2 §7.11.4, §7.9.6 (name trees).

## Cross-cutting invariants
- [Text string encoding](../invariant/text-string-encoding.md)
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md)
- [Tree traversal cycles](../invariant/tree-traversal-cycles.md)

## Blast radius
- [ZUGFeRD](zugferd.md) — `justpdf-special`'s invoice reader finds the XML by `FileSpec::filename` from `read_embedded_files` and reads it with `extract_file`; a change to the filename decoding or the MD5 refusal changes which invoices it finds.
- [Compress stripping](compress-stripping.md) — `strip_extras` deletes the whole `EmbeddedFiles` tree, which takes a Factur-X/ZUGFeRD invoice with it (#58).
- [Stream filters](stream-filters.md) — `extract_file` returns what `decode_stream` gives, unbounded in size (#161).
- [Sanitize](sanitize.md) — ADR 0005's attachments category removes this tree whole, plus `/AF` and FileAttachment annotations this module does not read (not built: #310).
- [Facade](facade.md) — `Document::embedded_files` returns `FileSpec`, so a field change is a public API change.

## Known holes / open
- There is no extraction round-trip test (only reading the MIME back after adding: `test_mime_type_is_written_once_escaped`).
- Adding to a tree whose root has `/Kids` puts the entry in the root's `/Names` (inferred).
- There is no depth limit: extreme depth without a cycle can overflow the stack (inferred). Tracked: #122
- Name-tree values are resolved outside the visit budget. Tracked: #135
- `extract_file` has no decoded-size bound. Tracked: #161
- File names are written as BOM-less UTF-8, and non-BOM strings read as lossy UTF-8. Tracked: #33
- `extreme` removes attachments, and `high` removes the PDF/A and Factur-X XMP (decided on #58: detect and preserve). Tracked: #58
