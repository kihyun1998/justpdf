# Repair

## What it is
For files with a broken xref, scans the whole file for `N M obj` headers to rebuild the xref, and finds the trailer or synthesizes one from the `/Type /Catalog` object.

## Governing decisions
**None.**

## Design model
- When the same number appears more than once, **the last one wins** (imitating incremental update semantics).
- The trailer search looks only at the last 4 KiB. If the version header cannot be read, it is set to 1.4.
- The only entries it creates are `InUse` — objects inside object streams are not recovered (inferred).
- `from_raw_parts` builds the document with `security: None` and does no encryption detection.

## Code
- `justpdf-core/src/repair.rs` — `rebuild_xref`, `repair_document`, `from_bytes_with_repair`, `scan_object_headers`, `find_trailer_dict`, `synthesise_trailer`, `try_parse_dict_at`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Xref](xref.md) — builds the same `Xref` structure.
- [Document access](document-access.md) — assembles a `PdfDocument` with `from_raw_parts`.
- [Tokenizer](tokenizer.md), [Object model](object-model.md) — used to scan headers and dictionaries.

## Known holes / open
- **Nothing calls it.** Check consumers with a command: `rg -l 'from_bytes_with_repair|repair_document|rebuild_xref' --glob '*.rs' --glob '!target' .` — only `repair.rs` itself shows up. A plain `open` does not fall back to repair on failure.
- A repaired encrypted document is not decrypted (Design model above).
