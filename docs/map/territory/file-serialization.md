# File serialization (header, xref, trailer)

## What it is
Writes an object list as a complete PDF file: the header, a binary marker, `N g obj` bodies, a classic xref table (or a `/Type /XRef` stream), and the trailer (`/Size /Root /Info`, plus `/Encrypt /ID` when encrypted) (inferred, `writer/serialize.rs — serialize_pdf_impl`, `serialize_with_xref_stream_impl`). `PdfWriter` allocates and holds object numbers and the generation of each held object (inferred, `writer/mod.rs`).

## Governing decisions
**None.** [ADR 0006](../../adr/0006-breaking-changes-are-allowed-before-1-0.md) sits adjacent: it does not decide file layout, but it removes "do not change a public signature" as a reason, and the two-set API below was built for that reason (#72, before the ADR).

## Design model
- **Two sets of entry points.** The public functions (`serialize_pdf`, `serialize_pdf_encrypted`, `serialize_pdf_with_xref_stream`, `write_xref_stream`) take only number/object slices, so they write every object at generation 0. The crate-internal functions that take a `PdfWriter` (`serialize_writer`, `serialize_writer_with_state`, `serialize_writer_encrypted`, `serialize_writer_with_xref_stream`, `write_xref_stream_with_generations`) write the object header, the xref entry and the encryption key at the writer's `generations` (inferred; #72, [Source generation](../invariant/source-generation.md)). The second set was added so that the public signatures would not change (technical decision, #72). `PdfWriter::write_to_bytes` is on the writer side too and writes no `/Info` (inferred).
- **Trailer references.** The writer-side functions take the catalog and info as **numbers**, not references, and write trailer `/Root` and `/Info` at the generation the writer writes that object at (`PdfWriter::reference_to`, #110). The public `write_to_bytes(catalog_ref)` ignores `catalog_ref`'s generation (inferred, `write_to_bytes`).
- `serialize_writer_with_xref_stream` rejects a compressed object, or an object stream, at generation ≠ 0 with `InvalidObject` ([Object streams](object-streams.md), #106) (inferred).
- **Xref stream field widths.** The width of the third field (`w3`) fits the largest of the object-stream index, 255 and the largest generation (inferred, `write_xref_stream_with_generations`). Fitting it without the generation truncated generation 300 to 44 in one byte (`test_build_with_xref_stream_keeps_a_generation_wider_than_a_byte`). `w2` fits the largest body offset and object-stream number (inferred). `bytes_needed` returns at most 4, so an offset past 4 GiB is truncated (inferred).
- **Free entries.** Gaps in the numbering are written as `0000000000 00000 f` in the table and as type 0 with field 3 = 0 in a stream, unlinked. Object 0 is `65535` in the table and `255` in a stream (inferred, `serialize_pdf_impl`, `write_xref_stream_with_generations`).
- **The encryption dictionary is not encrypted.** Exclusion is decided by **object number** only: `serialize_pdf_impl` compares each number with `encrypt_ref.obj_num`, and `encrypt_object` compares it with `SecurityState::encrypt_obj_num` (inferred). If another object had the `/Encrypt` dictionary's number, it would go out as plaintext.
- So that no number collides, `PdfWriter::set_object` at a number it does not hold raises `next_obj_num` above it (inferred, `set_object`; `test_set_object_at_a_new_number_is_not_reused`), and `serialize_writer_encrypted` adds the `/Encrypt` dictionary with `add_object` at a fresh number (inferred).
- **`/ID`** is written only on the encrypted path; an unencrypted file gets none (inferred, `serialize_pdf_impl`). `serialize_writer_encrypted` writes `/ID [permanent changing]` and derives the file key from the permanent identifier (inferred). The file ID rules belong to [Object encryption](object-encryption.md).
- **The xref stream path** raises the header version to at least 1.5 and **has no encryption path** (inferred, `serialize_with_xref_stream_impl`). It raises major and minor independently (`version.0.max(1)`, `version.1.max(5)`), so a version of 2.0 is written as `%PDF-2.5` (inferred).
- `serialize_pdf_impl` has an unused `_extra_trailer` parameter, so there is no channel for adding trailer keys (inferred).
- **Input size**: nothing here bounds the object count or number. `serialize_pdf_impl` writes one xref line for every number from 1 to the highest number, so a single object at number 4,000,000,000 gives a table of 4 billion lines (inferred).

## Code
- `justpdf-core/src/writer/serialize.rs` — `serialize_pdf`, `serialize_pdf_encrypted`, `serialize_writer`, `serialize_writer_with_state`, `serialize_writer_encrypted`, `serialize_pdf_impl`, `serialize_pdf_with_xref_stream`, `serialize_writer_with_xref_stream`, `serialize_with_xref_stream_impl`
- `justpdf-core/src/writer/object_stream.rs` — `write_xref_stream`, `write_xref_stream_with_generations`, `bytes_needed`, `write_field`
- `justpdf-core/src/writer/mod.rs` — `PdfWriter`, `add_object`, `alloc_object_num`, `set_object`, `write_to_bytes`, `write_to_file`, `generation`, `generation_of`, `reference_to`, `test_set_object_at_a_new_number_is_not_reused`

## Reference behaviour
**None.** No output of this module has been compared with a reference writer's. The clause to compare against is ISO 32000-2 §7.5 (file structure).

## Cross-cutting invariants
- [Object syntax roundtrip](../invariant/object-syntax-roundtrip.md) — every object goes through `serialize_object` and becomes a file here.
- [Xref entry format](../invariant/xref-entry-format.md) — the classic xref table in `serialize_pdf_impl`.
- [Source generation](../invariant/source-generation.md) — the writer-side functions write at the source generation; the public slice functions write 0.

## Blast radius
- [Xref](xref.md) — reads the xref and trailer written here. It assumes 20-byte table entries and stream field widths that hold every value.
- [Object serialization](object-serialization.md) — every body is that layer's output. A body that does not parse back makes the xref offsets point at garbage.
- [Object encryption](object-encryption.md) — `serialize_pdf_impl` decides which object is the `/Encrypt` dictionary by number and calls `encrypt_object_for_writing` for the rest. Both sides assume the number identifies the dictionary.
- [Document builder](document-builder.md), [Document modifier](document-modifier.md) — `build` ends in these functions. They assume the writer-side functions write trailer `/Root` and `/Info` at the writer's generation and that an encrypted save adds the `/Encrypt` object at a fresh number.
- [Object streams](object-streams.md), [Compress pipeline](compress-pipeline.md) — the only users of the xref stream path; the pipeline's call is commented out (inferred, `compress.rs`). They assume the generation-0 guard and the field widths.
- [Linearization](linearization.md) — writes its own xref and trailer next to these. A fix to the layout here does not reach it.

## Known holes / open
- An xref stream combined with encryption cannot be written (inferred, `serialize_with_xref_stream_impl` has no encryption parameter; `build_with_xref_stream` refuses it).
- `w2` is chosen from the largest body offset, but the xref stream's own entry is written with its offset (`buf.len()`, past every body). When that offset needs one more byte than the largest body offset, its entry is truncated (inferred). It parses today because `startxref` points straight at the stream.
- A version of 2.0 or higher with a minor version below 5 is written as `x.5` on the xref stream path (inferred, `serialize_with_xref_stream_impl`).
- No `/ID` is written without encryption. `/ID` is required in PDF 2.0 (Arlington model, `FileTrailer.tsv`, read on #77, 2026-10-10). The maintainer decided on 2026-10-10 to always write `/ID` on every complete file write (#77). Tracked: #77
- Should the two-set API (public slice functions at generation 0, internal writer functions at the source generation) stay, now that ADR 0006 says avoiding a public API break is no reason to keep a worse design? Only the maintainer can answer.
