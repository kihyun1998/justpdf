# Format detection

## What it is
Decides the format of an input file, as a `DocumentFormat`. There is an extension path (`detect_format`) and a byte path (`detect_format_from_bytes`). The CLI's `convert` dispatches on the extension path; nothing in the repo calls the byte path.

## Governing decisions
**None.** [ADR-0001](../../adr/0001-crates-split-by-dependency-layer.md) places the code in `justpdf-formats` but decides nothing about detection.

## Design model
Status: read from the source only.
- `detect_format` maps a lower-cased extension: `pdf`; `xps`, `oxps`; `epub`; `svg`, `svgz`; `docx`, `xlsx`, `pptx`; `cbz`; `mobi`, `prc`; `fb2`; `txt`, `text`, `md`, `rst`, `log` → plain text; anything else `Unknown`.
- `detect_format_from_bytes` knows only `%PDF` and an XML or `<svg` start with `<svg` in the first 1024 bytes. Any ZIP (`PK\x03\x04`) returns `Unknown` ("simplified: just return Unknown for ZIP, let the caller try each format"), so EPUB, Office, XPS and CBZ are never told apart by content. MOBI and FB2 are not recognised by content either.
- `.svgz` is detected as SVG, but `SvgDocument::open` reads the file as UTF-8 text, so a gzipped SVG fails: `I/O error: stream did not contain valid UTF-8` (measured 2026-10-30 at 88b2a08, `justpdf convert r.svgz`).
- Input bound: the byte path reads at most 1024 bytes; the extension path reads none.

## Code
- `justpdf-formats/src/detect.rs` — `DocumentFormat`, `detect_format`, `detect_format_from_bytes`, `impl Display for DocumentFormat`

## Reference behaviour
**None.**

## Cross-cutting invariants
**None.**

## Blast radius
- [CLI](cli.md) — `cmd_convert` matches on `detect_format` and has an arm for every variant except `Unknown` (verified 2026-10-30); a new variant or extension falls into `unsupported input format` until the CLI gains an arm.
- [Office input](office-input.md) — `OfficeDocument::open` re-derives DOCX/XLSX/PPTX from the extension on its own; the two extension lists must agree.

## Known holes / open
- A ZIP-based file with a wrong or missing extension is not identified, and the byte path gives up on every ZIP.
- `.svgz` is detected but cannot be read (Design model).
- `detect.rs` has no tests, and `detect_format_from_bytes` has no caller in the repo.
