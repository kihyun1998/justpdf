# Format detection

## What it is
Decides the format of an input file. There is an extension-based path (`detect_format`) and a byte-based path (`detect_format_from_bytes`).

## Governing decisions
**None.**

## Design model
- Byte-based detection returns `Unknown` for any ZIP ("simplified … let the caller try each format"). EPUB, Office, XPS and CBZ are all ZIPs, so in practice it relies on the extension.

## Code
- `justpdf-formats/src/detect.rs` — `detect_format`, `detect_format_from_bytes`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [CLI](cli.md) — `cmd_convert` branches on the detection result (the MOBI and FB2 branches are missing).
- [Format document](format-document.md) — picks the implementation for the detected format.

## Known holes / open
- A ZIP-based file with the wrong extension is not identified.
