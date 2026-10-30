# Compress stripping (metadata and extras)

## What it is
Two knobs, both handled by `strip_non_essential`. `strip_metadata` removes the catalog's `/Metadata`, `/StructTreeRoot`, `/OutputIntents`, `/PieceInfo` and `/MarkInfo`, and each page's `/Thumb`, `/PieceInfo` and `/LastModified`. `strip_extras` removes the `/EmbeddedFiles` and `/JavaScript` name trees from the catalog's `/Names`, and each page's `/AA`. The body of `strip_non_essential` owns the key lists; GC then removes the objects they referenced.

## Governing decisions
**None.** `dev/pdf-compress-wasm-design.md` §7 records "Structure tree removal — loses accessibility — high/extreme only", and Phase F lists the keys removed. It is a design doc, not a decision record; the code follows it (`preset_high` and `preset_extreme` set `strip_metadata`, read at 88b2a08).

## Design model
- Removing `/StructTreeRoot` and `/MarkInfo` makes a tagged PDF untagged, so it loses its accessibility structure. The design doc records this as accepted for `high` and `extreme` (above).
- Removing `/Metadata` and `/OutputIntents` removes the PDF/A identification and output intent, so a PDF/A file stops declaring conformance (measured on #58, below).
- Only the catalog's `/Metadata` goes. `/Metadata` streams on pages, images, fonts and other objects stay, and so does the trailer `/Info` dictionary (`DocumentModifier` keeps `info_num`; inferred, read at 88b2a08).
- `/LastModified` is removed from pages only; the doc comment's list ("PieceInfo, LastModified") reads as if the catalog's went too.
- `strip_extras` does not touch `/OpenAction`, the catalog `/AA`, `/Annots` (annotation actions and FileAttachment annotations), `/AcroForm` (field actions), `/AF`, `/OCProperties` or `/Outlines`.
- Pages are found by scanning every object for `/Type /Page`. Each key removed adds one to `metadata_items_stripped`, whichever knob removed it.
- **Input bounds**: one catalog, one `/Names` dictionary and one pass over the page dictionaries; nothing else.

## Code
- `justpdf-core/src/writer/compress.rs` — `strip_non_essential`, `create_pdf_with_metadata_and_jpeg`, `test_strip_metadata_low_noop`, `test_strip_metadata_high_valid`, `test_strip_extras_extreme_valid`, `test_strip_metadata_text_preserved`, `test_strip_preset_config`, `test_strip_metadata_images_preserved`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Embedded files](embedded-files.md) — `strip_extras` removes the `EmbeddedFiles` name tree that embedded-file listing reads; a ZUGFeRD invoice's XML is an embedded file, so this knob breaks [ZUGFeRD](zugferd.md) files.
- [Actions](actions.md) — the knob's promise to remove JavaScript covers only the `/JavaScript` name tree and page `/AA`; action types reachable elsewhere stay.
- [Document modifier](document-modifier.md) — stripping finds the catalog through `catalog_ref`, which holds only while no stage renumbers objects.
- [Compress presets](compress-presets.md) — `high` turns on `strip_metadata` and `extreme` adds `strip_extras`; the web's copy for Balanced claims a metadata strip `medium` does not do.
- [Sanitize](sanitize.md) — ADR 0005's metadata, attachment and navigation categories remove what this pass strips by size rather than by promise, and both must keep a PDF/A or PDF/UA declaration (the `extreme` preset breaking ZUGFeRD invoices is the same question, #58). Not built: #310.

## Known holes / open
- The promise to "remove" JavaScript does not cover JavaScript in `/OpenAction`, the catalog `/AA`, annotation actions or field actions (inferred, read at 88b2a08).
- Measured 2026-10-07 on #58, on a Factur-X-shaped file (EmbeddedFiles + `/AF` + XMP with `pdfaid:part=3` and the `fx:` schema + `/OutputIntents`): `high` already removes the XMP and `/OutputIntents` through `strip_metadata`, so the PDF/A and Factur-X declarations vanish; `extreme` also drops the EmbeddedFiles name tree, but `/AF` keeps the XML bytes in the file, hidden from attachment lists. Decided on #58 (triage, 2026-10-07): detect PDF/A (`pdfaid:part`), PDF/UA (`pdfuaid:part`) and catalog `/AF`; keep what that conformance needs (plus `/StructTreeRoot`/`/MarkInfo` for level A or PDF/UA) and report it in `CompressStats`; without conformance, drop `/AF` references with the name tree; and widen JavaScript removal to `/OpenAction`, `/AA` and annotation actions. Tracked: #58.
