# File serialization (header, xref, trailer)

## What it is
Writes an object list as a complete PDF file: header, binary marker, `N g obj` bodies, classic xref table (or a `/Type /XRef` stream), trailer (`/Size /Root /Info`, plus `/Encrypt /ID` when encrypted). `PdfWriter` handles allocating and holding object numbers.

## Governing decisions
**None.**

## Design model
- Generation numbers: the public functions (`serialize_pdf`, `serialize_pdf_encrypted`, `serialize_pdf_with_xref_stream`, `write_xref_stream`) take only number/object slices, so they write every object at generation 0. The crate-internal functions that take a `PdfWriter` (`serialize_writer`, `serialize_writer_with_state`, `serialize_writer_with_xref_stream`, `write_xref_stream_with_generations`) write the header, xref entry and encryption key at the writer's `generations` — there are two sets so as not to change the public signatures (#72, [Source generation](../invariant/source-generation.md)). `PdfWriter::write_to_bytes` is on the writer side too. The writer-side functions take the catalog and info as **numbers**, not references, and write trailer `/Root` and `/Info` at the generation the writer writes that object at (`reference_to`, #110) — the public `write_to_bytes(catalog_ref)` does not use `catalog_ref`'s generation. `serialize_writer_with_xref_stream` rejects compressed objects and object streams with generation ≠ 0 as `InvalidObject` ([Object streams](object-streams.md), #106).
- The width of the xref stream's third field (`w3`) fits the largest of the index, 255 and the maximum generation. Fitting it without the generation truncates generation 300 to 44 in one byte (`test_build_with_xref_stream_keeps_a_generation_wider_than_a_byte`).
- The encryption dictionary itself is not encrypted — exclusion from encryption is decided by **object number** only (`encrypt_obj_num`), so if the `/Encrypt` dictionary's number collides with another object, that object goes out as plaintext.
- When `PdfWriter::set_object` inserts a new number, it raises `next_obj_num` above it. If a number were allocated again, the number-based check above would send an object whose number collides with the `/Encrypt` dictionary out as plaintext in an encrypted save.
- The xref stream path raises the version to at least 1.5 and **has no encryption path**.
- `w2` in `write_xref_stream` is set from the largest body offset, but the xref stream's own offset can be larger (inferred: latent defect; it parses today because `startxref` points straight at the stream).
- `serialize_pdf_impl` has an unused `_extra_trailer` parameter — the channel for adding trailer keys is empty.

## Code
- `justpdf-core/src/writer/serialize.rs` — `serialize_pdf`, `serialize_pdf_encrypted`, `serialize_writer_encrypted`, `serialize_pdf_impl`, `serialize_pdf_with_xref_stream`, `serialize_writer`, `serialize_writer_with_state`, `serialize_writer_with_xref_stream`, `serialize_with_xref_stream_impl`
- `justpdf-core/src/writer/object_stream.rs` — `write_xref_stream`, `write_xref_stream_with_generations`, `bytes_needed`, `write_field`
- `justpdf-core/src/writer/mod.rs` — `PdfWriter`, `add_object`, `alloc_object_num`, `set_object`, `write_to_bytes`, `test_set_object_at_a_new_number_is_not_reused`, `generation`, `generation_of`, `reference_to`

## Reference behaviour
**None.** Clause to compare against: ISO 32000-2 §7.5 (file structure).

## Cross-cutting invariants
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md) — every object goes through `serialize_object` and becomes a file here.
- [Xref entry format](../invariant/xref-entry-format.md) — the classic xref table in `serialize_pdf_impl`.
- [Source generation](../invariant/source-generation.md) — writer-side functions write at the source generation; public slice functions write 0.

## Blast radius
- [Xref](xref.md) — the side that reads the xref and trailer written here.
- [Object serialization](object-serialization.md) — per-object output.
- [Object encryption](object-encryption.md) — `serialize_pdf_encrypted` calls `encrypt_object` for each object.
- [Document builder](document-builder.md), [Document modifier](document-modifier.md) — `build` ends in these functions. An encrypted save goes through `serialize_writer_encrypted` for both (adds the `/Encrypt` object, `/ID [permanent changing]`). The xref stream path (`serialize_pdf_with_xref_stream`) has no encryption.
- [Object streams](object-streams.md), [Compress pipeline](compress-pipeline.md) — the only consumers of the xref stream path (currently off).

## Known holes / open
- An xref stream combined with encryption cannot be written.
