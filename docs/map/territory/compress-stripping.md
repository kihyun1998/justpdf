# Compress stripping (metadata and extras)

## What it is
Two knobs. `strip_metadata` removes the document catalog's XMP, structure tree, output intents, PieceInfo and MarkInfo, page thumbnails and so on. `strip_extras` removes the embedded-files and JavaScript name trees and the page `/AA`. The body of `strip_non_essential` owns which keys are removed.

## Governing decisions
**None.**

## Design model
- It removes the structure tree (`StructTreeRoot`) and `MarkInfo`, so a tagged PDF loses its accessibility. There is no record of this choice being made.
- `strip_extras` does not touch `/OpenAction`, the catalog `/AA`, `/Annots`, `/AcroForm`, FileAttachment annotations, `/OCProperties` or `/Outlines`.

## Code
- `justpdf-core/src/writer/compress.rs` — `strip_non_essential`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Embedded files](embedded-files.md) — removes the `EmbeddedFiles` name tree. A ZUGFeRD invoice's XML is an embedded file, so this knob breaks [ZUGFeRD](zugferd.md) files.
- [Actions](actions.md) — the JavaScript name tree and `/AA`.
- [Compress presets](compress-presets.md) — which preset turns on which knob.

## Known holes / open
- The promise to "remove" JavaScript does not cover JS in `/OpenAction`, the catalog `/AA` or annotation actions.
- Measured 2026-10-07 on a Factur-X-shaped file (EmbeddedFiles + `/AF` + XMP with `pdfaid:part=3` and the `fx:` schema + `/OutputIntents`): `high` already removes the XMP and `/OutputIntents` through `strip_metadata`, so the PDF/A and Factur-X declarations vanish; `extreme` also drops the EmbeddedFiles name tree, but `/AF` keeps the XML bytes in the file, hidden from attachment lists. Decided in #58: detect PDF/A (`pdfaid:part`), PDF/UA (`pdfuaid:part`) and catalog `/AF`, keep what that conformance needs (plus `/StructTreeRoot`/`/MarkInfo` for level A or PDF/UA), report it in `CompressStats`; without conformance, drop `/AF` references with the name tree; and widen JavaScript removal to `/OpenAction`, `/AA` and annotation actions.
- Tracked: #58 (presets and conformance, JavaScript scope)
