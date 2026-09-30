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
- Tracked: #58 (extreme removes embedded files)
