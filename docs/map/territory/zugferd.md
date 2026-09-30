# ZUGFeRD (e-invoice reading)

## What it is
Finds ZUGFeRD/Factur-X XML among a PDF's attachments, detects the profile and parses the invoice information. Read-only (no generation).

## Governing decisions
- [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md)

## Design model
- Pulls the XML out with `read_embedded_files` and `extract_file` from core [Embedded files](embedded-files.md).

## Code
- `justpdf-special/src/zugferd/mod.rs` — `is_zugferd`, `extract_zugferd`, `parse_zugferd_xml`, `detect_profile`, `ZugferdProfile`, `ZugferdInfo`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [Embedded files](embedded-files.md) — the input path.
- [Compress stripping](compress-stripping.md) — the extreme preset deletes the attachments name tree, which makes a ZUGFeRD file no longer an invoice.

## Known holes / open
- No test runs the lookup against a real ZUGFeRD file.
- Tracked: #58 (extreme removes attachments)
