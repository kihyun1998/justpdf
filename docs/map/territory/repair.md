# Repair

## What it is
For a file whose xref or trailer is broken, scans the whole file for `N M obj` headers to rebuild the xref, and finds the last trailer or synthesizes one from a `/Type /Catalog` object. It is a public API of `justpdf-core` (`pub mod repair`; `PdfDocument::from_bytes_with_repair`) that no product path calls.

## Governing decisions
**None.**

## Design model
Rules read from the code (*inferred* unless marked).
- **Header scan** (`scan_object_headers`, `match_obj_header`): a header counts only at the start of a line (position 0 or after CR/LF), after optional spaces or tabs, as digits, one or more spaces, digits, one or more spaces, `obj`, then whitespace, `<`, `[`, `/` or EOF. Numbers that do not fit `u32` / `u16` are skipped. A header that follows other text on its line, or one separated by tabs or line breaks, is missed.
- When the same number appears more than once, **the last one wins** (`HashMap::insert` in file order), imitating incremental-update semantics.
- The only entries it creates are `InUse`. Objects inside object streams are not recovered, because the scan never opens a stream.
- **Trailer:** `find_trailer_dict` takes the last `trailer` keyword in the last 4 KiB whose following object parses as a dictionary. Without one, `synthesise_trailer` takes the first object, in `HashMap` order, whose dictionary has `/Type /Catalog`, and builds `/Root` and `/Size`. `/Size` is added when missing.
- If no version header is found in the first 1024 bytes, the version is 1.4 (`parse_version_tolerant`).
- `from_bytes_with_repair` and `repair_document` both try `PdfDocument::from_bytes` first and repair only when it fails; each clones the whole input for that first try.
- The repaired document is built with `PdfDocument::from_raw_parts`, which sets `security: None` and does no encryption detection.
- **Input bound:** one linear pass over the file. The entry map is bounded by the headers present; nothing else is bounded here. `synthesise_trailer` may parse every scanned object while looking for the catalog.

## Code
- `justpdf-core/src/repair.rs` — `rebuild_xref`, `repair_document`, `PdfDocument::from_bytes_with_repair`, `scan_object_headers`, `match_obj_header`, `find_trailer_dict`, `synthesise_trailer`, `try_parse_dict_at`, `parse_version_tolerant`
- `justpdf-core/src/parser.rs` — `PdfDocument::from_raw_parts`

## Reference behaviour
**None.** Not compared with another reader's reconstruction.

## Cross-cutting invariants
**None.**

## Blast radius
- [Xref](xref.md) — builds the same `Xref` shape. A field or variant added there must be produced here, or a repaired document lacks it.
- [Document access](document-access.md) — assembles a `PdfDocument` through `from_raw_parts`. A field added to `PdfDocument` must be initialised there, and the encryption state it skips is what `resolve` relies on to decrypt.
- [Tokenizer](tokenizer.md), [Object model](object-model.md) — parse the trailer and candidate catalog dictionaries.

## Known holes / open
- **Nothing calls it.** `rg -l 'from_bytes_with_repair|repair_document|rebuild_xref' --glob '*.rs' --glob '!target' .` finds only `repair.rs` (2026-10-30). A plain `open` does not fall back to repair. #109's decision would build a header index from this scan for `resolve`. Tracked: #109
- A repaired encrypted document is not decrypted: `security` is `None`, so `resolve` returns encrypted strings and streams as they are, and `is_encrypted()` is false, so callers that refuse encrypted documents (`compress_pdf`, `DocumentModifier::from_document`) would process it (inferred).
- Objects inside object streams are lost (above).
- With several catalogs and no trailer, which one becomes `/Root` depends on `HashMap` iteration order (inferred).
